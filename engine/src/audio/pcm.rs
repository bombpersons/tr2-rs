//! Minimal PCM16 WAV asset loader. Extraction emits only this portable format.
//! This avoids a general-purpose codec dependency for already decoded audio.
use std::{num::NonZero,sync::Arc,time::Duration};
use bevy::{asset::{AssetLoader,LoadContext,io::Reader},audio::{ChannelCount,Decodable,SampleRate,Source},prelude::*};

#[derive(Asset,TypePath)]
pub struct PcmAudio { bytes:Arc<[u8]>, channels:ChannelCount, rate:SampleRate }
pub struct PcmDecoder {audio:PcmAudio,offset:usize}
impl Iterator for PcmDecoder {
    type Item=f32;
    fn next(&mut self)->Option<f32>{let s=self.audio.bytes.get(self.offset..self.offset+2)?;self.offset+=2;Some(f32::from(i16::from_le_bytes([s[0],s[1]]))/32768.0)}
}
impl Source for PcmDecoder {
    fn current_span_len(&self)->Option<usize>{Some((self.audio.bytes.len()-self.offset)/2)}
    fn channels(&self)->ChannelCount {self.audio.channels}
    fn sample_rate(&self)->SampleRate {self.audio.rate}
    fn total_duration(&self)->Option<Duration>{Some(Duration::from_secs_f64(self.audio.bytes.len() as f64/2.0/f64::from(self.audio.channels.get())/f64::from(self.audio.rate.get())))}
}
impl Decodable for PcmAudio {
    type Decoder=PcmDecoder;
    fn decoder(&self)->PcmDecoder {PcmDecoder{audio:Self{bytes:self.bytes.clone(),channels:self.channels,rate:self.rate},offset:0}}
}
#[derive(Default,TypePath)]
pub struct PcmLoader;
impl AssetLoader for PcmLoader {
    type Asset=PcmAudio; type Settings=(); type Error=std::io::Error;
    async fn load(&self,r:&mut dyn Reader,_:&(),_:&mut LoadContext<'_>)->Result<PcmAudio,Self::Error>{
        let mut b=Vec::new();r.read_to_end(&mut b).await?;
        let bad=||std::io::Error::new(std::io::ErrorKind::InvalidData,"Expected extracted PCM16 RIFF/WAVE asset");
        if b.get(..4)!=Some(b"RIFF")||b.get(8..12)!=Some(b"WAVE"){return Err(bad());}
        let mut i=12;let mut format=None;let mut data=None;
        while let Some(header)=b.get(i..i+8) {
            let size=u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;i+=8;
            let chunk=b.get(i..i+size).ok_or_else(bad)?;
            if &header[..4]==b"fmt "&&chunk.len()>=16 {
                let tag=u16::from_le_bytes(chunk[..2].try_into().unwrap());
                let channels=u16::from_le_bytes(chunk[2..4].try_into().unwrap());
                let rate=u32::from_le_bytes(chunk[4..8].try_into().unwrap());
                let bits=u16::from_le_bytes(chunk[14..16].try_into().unwrap());
                if tag!=1||bits!=16{return Err(bad());}
                format=Some((NonZero::new(channels).ok_or_else(bad)?,NonZero::new(rate).ok_or_else(bad)?));
            }else if &header[..4]==b"data" {data=Some(Arc::<[u8]>::from(chunk));}
            i+=size+size%2;
        }
        let (channels,rate)=format.ok_or_else(bad)?;let bytes=data.ok_or_else(bad)?;
        if bytes.len()%(usize::from(channels.get())*2)!=0{return Err(bad());}
        Ok(PcmAudio{bytes,channels,rate})
    }
    fn extensions(&self)->&[&str]{&["wav"]}
}
