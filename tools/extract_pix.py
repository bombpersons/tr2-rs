#!/usr/bin/env python3
"""Convert headerless RNC2-compressed TR2 PSX RGB555 screens to PNG.
Reference: OpenLara texture.h LoadRNC and original RNC2 bitstream algorithm.
"""
import argparse,json,struct,hashlib
from pathlib import Path
from PIL import Image
class Bits:
    def __init__(self,b):self.b=b;self.p=0;self.n=0;self.v=0
    def byte(self):
        if self.p>=len(self.b):raise EOFError(self.p)
        v=self.b[self.p];self.p+=1;return v
    def bit(self):
        if not self.n:self.v=self.byte();self.n=8
        self.n-=1;return (self.v>>self.n)&1
    def bits(self,n):
        v=0
        for _ in range(n):v=(v<<1)|self.bit()
        return v
    def offset(self):
        v=0
        if self.bit():
            v=self.bit()
            if self.bit():
                v=((v<<1)|self.bit())|4
                if not self.bit():v=(v<<1)|self.bit()
            elif not v:v=self.bit()+2
        return ((v<<8)|self.byte())+1

def decompress(b):
    size=2048*2048*2
    if b[:4]==b'RNC\x02':
        size,csize=struct.unpack_from('>2I',b,4);b=b[18:18+csize]
    bs=Bits(b);bs.bits(2);out=bytearray()
    while bs.p<len(b) and len(out)<size:
        if not bs.bit():out.append(bs.byte());continue
        if bs.bit():
            if bs.bit():
                if bs.bit():
                    n=bs.byte()+8
                    if n==8:
                        if not bs.bit():break
                        continue
                else:n=3
                off=bs.offset()
            else:n=2;off=bs.byte()+1
        else:
            n=bs.bit()+4
            if bs.bit():n=((n-1)<<1)+bs.bit()
            if n==9:
                for _ in range(bs.bits(4)*4+12):out.append(bs.byte())
                continue
            off=bs.offset()
        assert 0<off<=len(out),(bs.p,off,len(out))
        for _ in range(n):out.append(out[-off])
    assert len(out)%512==0,(len(out),bs.p)
    return out,bs.p

def main(src,dst):
    report=[];dst.mkdir(parents=True,exist_ok=True)
    for d in ('PIX','PIXUS'):
        for f in sorted((src/d).glob('*.RAW')):
            b=f.read_bytes();raw,parsed=decompress(b);width=len(raw)//512
            colors=bytearray()
            for c, in struct.iter_unpack('<H',raw):colors.extend(((c&31)*255//31,((c>>5)&31)*255//31,((c>>10)&31)*255//31))
            name=f'{d.lower()}_{f.stem.lower()}.png';im=Image.frombytes('RGB',(width,256),bytes(colors));im.save(dst/name)
            report.append(dict(source=f'{d}/{f.name}',path=name,width=width,height=256,compressed_bytes=len(b),decoded_bytes=len(raw),parsed_bytes=parsed,source_sha256=hashlib.sha256(b).hexdigest()))
            print(f'{f}: {width}x256 ({len(b)}->{len(raw)} bytes)')
    (dst/'manifest.json').write_text(json.dumps(report,indent=2))
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--input',type=Path,default=Path('research/disc'));p.add_argument('--output',type=Path,default=Path('game/assets/screens'));a=p.parse_args();main(a.input,a.output)
