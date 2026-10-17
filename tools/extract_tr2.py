#!/usr/bin/env python3
"""Decode TR2 PlayStation levels into standalone JSON/PNG assets.
Format cross-checked against OpenLara src/format.h; no engine code copied.
"""
from __future__ import annotations
import argparse, hashlib, json, math, struct, traceback, wave
from pathlib import Path
from PIL import Image

class Reader:
    def __init__(self, data: bytes, name='data'): self.data,self.p,self.name=data,0,name
    def read(self, fmt):
        s=struct.Struct('<'+fmt)
        if self.p+s.size>len(self.data): raise ValueError(f'{self.name}: {self.p:#x}+{s.size} exceeds {len(self.data):#x}')
        a=s.unpack_from(self.data,self.p); self.p+=s.size
        return a[0] if len(a)==1 else list(a)
    def raw(self,n):
        if n<0 or self.p+n>len(self.data): raise ValueError(f'{self.name}: invalid raw length {n} at {self.p:#x}')
        a=self.data[self.p:self.p+n];self.p+=n;return a
    def skip(self,n): self.raw(n)
    def array(self,fmt,count=None):
        if count is None: count=self.read('I')
        if count>2000000: raise ValueError(f'{self.name}: unreasonable array {count} at {self.p:#x}')
        return [self.read(fmt) for _ in range(count)]

def room(r:Reader,idx:int):
    offset=r.p;x,z,bottom,top=r.read('4i');size=r.read('I');start=r.p;end=start+size*2
    vs=[]
    # The vertex count is stored twice (RAM 0x800f4144/6 for Wall room 35).
    # Each vertex word is xyz in its low 15 bits (x 10-14, y 5-9, z 0-4) and
    # a 16-bit shade in its high half, as the room renderer 0x800617b0 reads it.
    count,again=r.read('2H');assert count==again,(idx,count,again)
    for _ in range(count):
        c=r.read('I');vs.append([((c>>10)&31)*1024,((c>>5)&31)*256+top,(c&31)*1024,c>>16,0])
    n=r.read('H');tex=r.array('H',n)
    if (r.p-start)%4:r.skip(2)
    qs=[]
    for t in tex:
        a,b,c,d=r.read('4H');qs.append([a>>2,b>>2,d>>2,c>>2,t&32767])
    n=r.read('H');r.skip(2);ts=[]
    for _ in range(n):
        t,a,b,c=r.read('4H');ts.append([a>>2,b>>2,c>>2,t&32767])
    if r.p>end: raise ValueError(f'room {idx} geometry overruns {r.p:#x}>{end:#x}')
    for f in qs+ts:
        assert all(v<len(vs) for v in f[:-1]),(idx,len(vs),f)
    r.raw(end-r.p)
    portals=[]
    for _ in range(r.read('H')):
        ri=r.read('H');normal=r.read('3h');pv=[]
        for j in range(4):
            a,b,c=r.read('3h');pv.append([a,b+top,c])
        portals.append(dict(room=ri,normal=normal,vertices=pv))
    nz,nx=r.read('2H');sectors=[]
    for _ in range(nx*nz):
        fi,box,below,floor,above,ceil=r.read('HHBbBb')
        sectors.append(dict(floor_data=fi,box=box,below=below,floor=floor*256,above=above,ceiling=ceil*256))
    ambient=r.read('3H');lights=r.array('3i2H2I',r.read('H'))
    placements=[]
    for _ in range(r.read('H')):
        a,b,c,rot,l1,l2,mid=r.read('3i4H');placements.append(dict(position=[a,b,c],rotation=rot,light=l2,mesh_id=mid))
    alternate,flags=r.read('hH')
    return dict(id=idx,source_offset=offset,origin=[x,0,z],bottom=bottom,top=top,vertices=vs,quads=qs,triangles=ts,portals=portals,x_sectors=nx,z_sectors=nz,sectors=sectors,ambient=ambient,lights=lights,static_placements=placements,alternate=alternate,flags=flags)

def mesh(data,offset):
    r=Reader(data,'mesh');r.p=offset
    center=r.read('3h');radius,flags,n=r.read('hHh');vs=r.array('4h',abs(n))
    normals=r.array('4h',abs(n)) if n>0 else []
    if n<0:
        for v in vs:v[3]=r.read('H')
    if n>0:
        r.skip(r.read('H')*12);r.skip(r.read('H')*10)
    qs=[];ts=[]
    for _ in range(r.read('H')):
        a,b,c,d,t=r.read('5H');qs.append([a>>3,b>>3,c>>3,d>>3,t&32767])
    for _ in range(r.read('H')):
        a,b,c,t=r.read('4H');ts.append([a>>3,b>>3,c>>3,t&32767])
    for f in qs+ts:assert all(v<len(vs) for v in f[:-1]),(offset,n,f)
    return dict(source_offset=offset,center=center,radius=radius,flags=flags,vertices=vs,normals=normals,quads=qs,triangles=ts)

def colors(c):
    # PlayStation RGB555: zero is transparent; opaque black uses STP bit.
    return [(v&31)*255//31 for v in (c,c>>5,c>>10)]+[255 if c else 0]

def atlas(textures,tiles,cluts,out,name="atlas.png"):
    # Deduplicated tile/CLUT pages retain exact UV coordinates and nearest sampling.
    # Cropping every texture rectangle instead keeps atlas compact and supports many palettes.
    images=[]
    for t in textures:
        uv=t['uv']; valid=uv[:3] if uv[3]==[0,0] else uv
        minx=min(v[0] for v in valid);maxx=max(v[0] for v in valid)
        miny=min(v[1] for v in valid);maxy=max(v[1] for v in valid)
        w,h=maxx-minx+1,maxy-miny+1
        assert t['tile']<len(tiles) and t['clut']<len(cluts),(t,len(tiles),len(cluts))
        src=tiles[t['tile']];pal=[colors(c) for c in cluts[t['clut']]]
        im=Image.new('RGBA',(w+2,h+2));pix=im.load()
        for y in range(h+2):
            yy=miny+max(0,min(h-1,y-1))
            for x in range(w+2):
                xx=minx+max(0,min(w-1,x-1));b=src[(yy*256+xx)//2];pix[x,y]=tuple(pal[(b>>(4*(xx%2)))&15])
        images.append((im,minx,miny))
    width=2048;positions=[];x=y=rowh=0
    for im,_,_ in images:
        if x+im.width>width:x=0;y+=rowh;rowh=0
        positions.append((x,y));x+=im.width;rowh=max(rowh,im.height)
    height=2**max(1,(y+rowh-1).bit_length());dst=Image.new('RGBA',(width,height))
    for t,(im,mx,my),(x,y) in zip(textures,images,positions):
        dst.paste(im,(x,y));t['atlas_uv']=[[(x+1+u-mx+0.5)/width,(y+1+v-my+0.5)/height] for u,v in t['uv']]
    dst.save(out/name);return [width,height]

def signed16(x): return x-65536 if x>=32768 else x

def pose(frames,offset,count):
    """Return original-unit bounds/root and Euler XYZ angles in 1/1024 turns."""
    i=offset//2
    assert 0<=i and i+9<=len(frames),(offset,count,len(frames))
    head=[signed16(x) for x in frames[i:i+9]];i+=9;angles=[]
    for _ in range(count):
        a=frames[i];i+=1;axis=a>>14
        if axis:
            v=[0,0,0];v[axis-1]=a&1023
        else:
            b=frames[i];i+=1;v=[(a&0x3ff0)>>4,((a&15)<<6)|((b&0xfc00)>>10),b&1023]
        angles.append(v)
    return dict(bounds=head[:6],root=head[6:9],angles=angles)

def skeleton(model,nodes):
    result=[dict(parent=-1,translation=[0,0,0])];parent=0;stack=[]
    for j in range(1,model['mesh_count']):
        i=model['node_offset']+(j-1)*4
        if i+4>len(nodes):
            # Some inventory meshes intentionally have no bone tree.
            result.append(dict(parent=0,translation=[0,0,0]));continue
        flags,x,y,z=nodes[i:i+4]
        if flags&1: parent=stack.pop()
        if flags&2: stack.append(parent)
        result.append(dict(parent=parent,translation=[x,y,z]));parent=j
    return result

def enrich_animation(models,animations,frames,nodes):
    starts=sorted(set(m['animation'] for m in models if m['animation']!=65535))
    for m in models:
        m['joints']=skeleton(m,nodes)
        m['initial_pose']=pose(frames,m['frame_offset'],m['mesh_count'])
        start=m['animation']
        if start==65535:continue
        end=next((i for i in starts if i>start),len(animations))
        m['animation_end']=end
        for a in animations[start:end]:
            count=(a['frame_end']-a['frame_start'])//max(1,a['frame_rate'])+1
            a['poses']=[pose(frames,a['frame_offset']+i*a['frame_size']*2,m['mesh_count']) for i in range(count)]

def decode_adpcm(data):
    coeff=[(0,0),(60,0),(115,-52),(98,-55),(122,-60)];h1=h2=0;pcm=[]
    assert len(data)%16==0
    for pos in range(0,len(data),16):
        header,flags=data[pos:pos+2];shift=header&15;filt=header>>4
        assert filt<=4,(pos,header)
        a,b=coeff[filt]
        for byte in data[pos+2:pos+16]:
            for nibble in (byte&15,byte>>4):
                if nibble>=8:nibble-=16
                v=((nibble<<12)>>shift)+((h1*a+h2*b+32)>>6)
                v=max(-32768,min(32767,v));pcm.append(v);h2=h1;h1=v
        if flags&1:break
    return pcm

def audio_assets(sound,offs,out):
    audio=out/'audio';audio.mkdir(exist_ok=True);records=[]
    for i,(a,b) in enumerate(zip(offs,offs[1:])):
        pcm=decode_adpcm(sound[a-offs[0]:b-offs[0]])
        name=f'audio/{i:03d}.wav'
        with wave.open(str(out/name),'wb') as wav:
            wav.setnchannels(1);wav.setsampwidth(2);wav.setframerate(8000)
            wav.writeframes(struct.pack('<'+'h'*len(pcm),*pcm))
        records.append(dict(index=i,path=name,sample_rate=8000,samples=len(pcm)))
    return records


def decode(path,out):
    b=path.read_bytes();r=Reader(b,str(path));out.mkdir(parents=True,exist_ok=True)
    ns=r.read('I');offs=r.array('I',ns+1);sound=r.raw(offs[-1]-offs[0])
    # Retain separated original ADPCM samples; WAV decoding is handled below.
    (out/'audio.vag').write_bytes(sound)
    r.skip(4)
    rooms=[room(r,i) for i in range(r.read('H'))]
    floors=r.array('H');mesh_data=r.raw(r.read('I')*2);mesh_offs=r.array('I')
    ak=['frame_offset','frame_rate','frame_size','state','speed','accel','frame_start','frame_end','next_animation','next_frame','state_count','state_offset','command_count','command_offset']
    animations=[dict(zip(ak,r.read('IBBHii8H'))) for _ in range(r.read('I'))]
    states=r.array('3H');ranges=r.array('4h');commands=r.array('h');nodes=r.array('i');frames=r.array('H')
    models=[]
    for _ in range(r.read('I')):
        typeid,pad,mc,ms,node,frame,anim,pad=r.read('4H2I2H');models.append(dict(id=typeid,mesh_count=mc,mesh_start=ms,node_offset=node,frame_offset=frame,animation=anim))
    static=[]
    for _ in range(r.read('I')):
        a=r.read('IH12hH');static.append(dict(id=a[0],mesh=a[1],visual_box=a[2:8],collision_box=a[8:14],flags=a[14]))
    tiles=[r.raw(32768) for _ in range(r.read('I'))];cluts=r.array('16H');r.skip(4)
    textures=[]
    for _ in range(r.read('I')):
        x0,y0,clut,x1,y1,tile,x2,y2,unk,x3,y3,attr=r.read('BBHBBHBBHBBH')
        textures.append(dict(tile=tile&16383,clut=clut,uv=[[x0,y0],[x1,y1],[x2,y2],[x3,y3]],attribute=attr))
    sprites=r.array('4h2H4B');sprite_sequences=r.array('Ihh');cameras=r.array('3ihH');sound_sources=r.array('3i2H');boxes=r.array('4BhH');overlaps=r.array('H');zones=[r.array('h',len(boxes)) for _ in range(10)];anim_tex=r.array('H')
    entities=[]
    for _ in range(r.read('I')):
        ty,ro,x,y,z,ang,l1,l2,fl=r.read('Hh3iHhhH');entities.append(dict(id=ty,room=ro,position=[x,y,z],rotation=ang,intensity=l1,intensity2=l2,flags=fl))
    r.skip(4);sound_map=r.array('h',370);sound_info=r.array('4H');r.skip(4);camera_frames=r.array('8h',r.read('H'))
    demo_data=list(r.raw(r.read('I')))
    assert r.p==len(b),(path,r.p,len(b))
    unique=sorted(set(mesh_offs));meshes=[mesh(mesh_data,o) for o in unique];lookup={o:i for i,o in enumerate(unique)};mesh_indices=[lookup[o] for o in mesh_offs]
    atlas_size=atlas(textures,tiles,cluts,out)
    sprite_textures=[]
    for left,top,right,bottom,clut,tile,u0,v0,u1,v1 in sprites:
        sprite_textures.append(dict(bounds=[left,top,right,bottom],tile=tile,clut=clut,uv=[[u0,v0],[u1,v0],[u0,v1],[u1,v1]],attribute=1))
    sprite_atlas_size=atlas(sprite_textures,tiles,cluts,out,'sprite_atlas.png')
    enrich_animation(models,animations,frames,nodes)
    audio_records=audio_assets(sound,offs,out)
    (out/'unparsed_tail.bin').write_bytes(b[r.p:])
    level=dict(schema=1,name=path.stem,source_sha256=hashlib.sha256(b).hexdigest(),source_size=len(b),parsed_bytes=r.p,trailing_bytes=len(b)-r.p,coordinate_system='original TR2: x right, y down, z forward; sector=1024',rooms=rooms,floor_data=floors,meshes=meshes,mesh_indices=mesh_indices,models=models,static_meshes=static,animations=animations,animation_states=states,animation_ranges=ranges,animation_commands=commands,nodes=nodes,frame_data=frames,textures=textures,atlas_size=atlas_size,sprites=sprites,sprite_sequences=sprite_sequences,cameras=cameras,sound_sources=sound_sources,boxes=boxes,overlaps=overlaps,zones=zones,animated_textures=anim_tex,entities=entities,sound_map=sound_map,sound_info=sound_info,sound_offsets=offs,camera_frames=camera_frames,audio=audio_records,demo_data=demo_data,sprite_textures=sprite_textures,sprite_atlas_size=sprite_atlas_size)
    validate(level)
    (out/'level.json').write_text(json.dumps(level,separators=(',',':')))
    return dict(name=path.stem,rooms=len(rooms),meshes=len(meshes),models=len(models),textures=len(textures),entities=len(entities),animations=len(animations),parsed_bytes=r.p,source_size=len(b),sha256=level['source_sha256'])

def validate(l):
    nr=len(l['rooms']);nt=len(l['textures']);nm=len(l['mesh_indices'])
    for room in l['rooms']:
        assert room['alternate']==-1 or 0<=room['alternate']<nr
        for p in room['portals']:assert p['room']<nr
        for s in room['sectors']:
            assert s['floor_data']<len(l['floor_data'])
            assert s['above']==255 or s['above']<nr
            assert s['below']==255 or s['below']<nr
        for f in room['quads']+room['triangles']:assert f[-1]<nt
    for m in l['meshes']:
        for f in m['quads']+m['triangles']:assert f[-1]<nt
    for m in l['models']:assert m['mesh_start']+m['mesh_count']<=nm
    for e in l['entities']:assert 0<=e['room']<nr

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--input',type=Path,default=Path('research/disc/DATA'));p.add_argument('--output',type=Path,default=Path('research/extracted/levels'));p.add_argument('--level');a=p.parse_args();report=[]
    files=[a.input/f'{a.level.upper()}.PSX'] if a.level else sorted(a.input.glob('*.PSX'))
    for f in files:
        try:
            v=decode(f,a.output/f.stem.lower());report.append(v);print(json.dumps(v),flush=True)
        except Exception:
            traceback.print_exc();raise
    a.output.mkdir(parents=True,exist_ok=True);(a.output/'manifest.json').write_text(json.dumps(report,indent=2))
