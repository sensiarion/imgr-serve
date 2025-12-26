use crate::image_types::Extensions;

/// it may be uuid, or complex link with path, either will work as simple string
pub type ImageId = String;

#[derive(Clone)]
pub struct ImageContainer {
    pub data: Box<Vec<u8>>,
    pub filename: Option<String>,
    pub extension: Extensions,
}


impl ImageContainer{
    pub fn new(data:Box<Vec<u8>>, filename:Option<String>, extension: Extensions)->Self{
        ImageContainer{
            data,
            filename,
            extension
        }
    }
}