use std::{
    collections::HashMap,
    io::{Read, Write},
    path::PathBuf,
};

pub trait ToFromBytes: Sized {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write;
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read;
    fn to_bytes_vec(&self) -> Vec<u8> {
        let mut b = Vec::new();
        _ = self.to_bytes(&mut b);
        b
    }
}

// impl ToFromBytes

// common types (String, Vec, ...)

impl ToFromBytes for String {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        self.len().to_bytes(s)?;
        s.write_all(self.as_bytes())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let len = ToFromBytes::from_bytes(s)?;
        let mut buf = vec![0; len];
        s.read_exact(&mut buf)?;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }
}
impl ToFromBytes for PathBuf {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        self.to_string_lossy().into_owned().to_bytes(s)
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        Ok(String::from_bytes(s)?.into())
    }
}

impl<C> ToFromBytes for Vec<C>
where
    C: ToFromBytes,
{
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        self.len().to_bytes(s)?;
        for elem in self {
            elem.to_bytes(s)?;
        }
        Ok(())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let len = ToFromBytes::from_bytes(s)?;
        let mut buf = Vec::with_capacity(len);
        for _ in 0..len {
            buf.push(ToFromBytes::from_bytes(s)?);
        }
        Ok(buf)
    }
}
impl<A> ToFromBytes for Option<A>
where
    A: ToFromBytes,
{
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        match self {
            None => s.write_all(&[0b11001100]),
            Some(v) => {
                s.write_all(&[0b00111010])?;
                v.to_bytes(s)
            }
        }
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let mut b = [0u8];
        s.read_exact(&mut b)?;
        match b[0] {
            0b00111010 => Ok(Some(ToFromBytes::from_bytes(s)?)),
            _ => Ok(None),
        }
    }
}
impl<K, V> ToFromBytes for HashMap<K, V>
where
    K: ToFromBytes + std::cmp::Eq + std::hash::Hash,
    V: ToFromBytes,
{
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        self.len().to_bytes(s)?;
        for (key, val) in self.iter() {
            key.to_bytes(s)?;
            val.to_bytes(s)?;
        }
        Ok(())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let len = ToFromBytes::from_bytes(s)?;
        let mut o = Self::with_capacity(len);
        for _ in 0..len {
            o.insert(ToFromBytes::from_bytes(s)?, ToFromBytes::from_bytes(s)?);
        }
        Ok(o)
    }
}

// - for (i/u)(size/8/16/32/64/128)

impl ToFromBytes for usize {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        (*self as u64).to_bytes(s)
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        Ok(u64::from_bytes(s)? as _)
    }
}
impl ToFromBytes for isize {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        (*self as i64).to_bytes(s)
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        Ok(i64::from_bytes(s)? as _)
    }
}
impl ToFromBytes for u8 {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        s.write_all(&[*self])
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let mut b = [0; 1];
        s.read_exact(&mut b)?;
        Ok(b[0])
    }
}
impl ToFromBytes for i8 {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        s.write_all(&self.to_be_bytes())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let mut b = [0; 1];
        s.read_exact(&mut b)?;
        Ok(Self::from_be_bytes(b))
    }
}
impl ToFromBytes for u16 {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        s.write_all(&self.to_be_bytes())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let mut b = [0; 2];
        s.read_exact(&mut b)?;
        Ok(Self::from_be_bytes(b))
    }
}
impl ToFromBytes for i16 {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        s.write_all(&self.to_be_bytes())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let mut b = [0; 2];
        s.read_exact(&mut b)?;
        Ok(Self::from_be_bytes(b))
    }
}
impl ToFromBytes for u32 {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        s.write_all(&self.to_be_bytes())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let mut b = [0; 4];
        s.read_exact(&mut b)?;
        Ok(Self::from_be_bytes(b))
    }
}
impl ToFromBytes for i32 {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        s.write_all(&self.to_be_bytes())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let mut b = [0; 4];
        s.read_exact(&mut b)?;
        Ok(Self::from_be_bytes(b))
    }
}
impl ToFromBytes for u64 {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        s.write_all(&self.to_be_bytes())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let mut b = [0; 8];
        s.read_exact(&mut b)?;
        Ok(Self::from_be_bytes(b))
    }
}
impl ToFromBytes for i64 {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        s.write_all(&self.to_be_bytes())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let mut b = [0; 8];
        s.read_exact(&mut b)?;
        Ok(Self::from_be_bytes(b))
    }
}
impl ToFromBytes for u128 {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        s.write_all(&self.to_be_bytes())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let mut b = [0; 16];
        s.read_exact(&mut b)?;
        Ok(Self::from_be_bytes(b))
    }
}
impl ToFromBytes for i128 {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        s.write_all(&self.to_be_bytes())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        let mut b = [0; 16];
        s.read_exact(&mut b)?;
        Ok(Self::from_be_bytes(b))
    }
}
