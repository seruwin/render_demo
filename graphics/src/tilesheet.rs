use crate::{AtlasGroup, GpuRenderer, Texture};
use image::{self, EncodableLayout, ImageBuffer, RgbaImage};

//used to map the tile in the tilesheet back visually
//this is only needed for the Editor.
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub layer: usize,
    pub id: u32,
}

//We can use this for editor loading and just as a precursor.
pub struct TileSheet {
    pub tiles: Vec<Tile>,
}

impl TileSheet {
    pub fn new(
        texture: Texture,
        renderer: &GpuRenderer,
        atlas: &mut AtlasGroup,
        tilesize: u32,
    ) -> Option<TileSheet> {
        let tilecount =
            (texture.size().0 / tilesize) * (texture.size().1 / tilesize);
        let sheet_width = texture.size().0 / tilesize;
        let sheet_height = texture.size().1 / tilesize;
        let atlas_width = atlas.atlas.extent.width / tilesize;
        let mut tilesheet = TileSheet {
            tiles: Vec::with_capacity(tilecount as usize),
        };
        let bytes = texture.bytes();

        // lets check this to add in the empty tile set first if nothing else yet exists.
        // Also lets add the black tile.
        if atlas.atlas.cache.len() == 0 {
            let image: RgbaImage = ImageBuffer::new(tilesize, tilesize);
            atlas.upload(
                "Empty".to_owned(),
                image.as_bytes(),
                tilesize,
                tilesize,
                0,
                renderer,
            )?;

            let mut image: RgbaImage = ImageBuffer::new(tilesize, tilesize);

            for (_x, _y, pixel) in image.enumerate_pixels_mut() {
                *pixel = image::Rgba([0, 0, 0, 255]);
            }

            atlas.upload(
                "black".to_owned(),
                image.as_bytes(),
                tilesize,
                tilesize,
                0,
                renderer,
            )?;
        }

        for id in 0..tilecount {
            let mut image: RgbaImage = ImageBuffer::new(tilesize, tilesize);
            // get its location to remap it back visually.
            let (tilex, tiley) = (
                ((id % sheet_width) * tilesize),
                ((id / sheet_height) * tilesize),
            );

            // lets create the tile from the texture.
            for y in 0..tilesize {
                for x in 0..tilesize {
                    let pos = (((id % sheet_width) * tilesize + (x * 4))
                        * ((id / sheet_height) * tilesize + y))
                        as usize;
                    let pixel = image::Rgba::<u8>([
                        bytes[pos],
                        bytes[pos + 1],
                        bytes[pos + 2],
                        bytes[pos + 3],
                    ]);
                    image.put_pixel(x, y, pixel);
                }
            }

            if image.enumerate_pixels().all(|p| p.2 .0[3] == 0) {
                // lets use our only Blank tile. this will always be the first loaded.
                tilesheet.tiles.push(Tile {
                    x: tilex,
                    y: tiley,
                    layer: 0,
                    id: 0,
                })

                // lets make sure its not pure black too.
            } else if image.enumerate_pixels().all(|p| {
                p.2 .0[0] == 0
                    && p.2 .0[1] == 0
                    && p.2 .0[2] == 0
                    && p.2 .0[4] == 255
            }) {
                tilesheet.tiles.push(Tile {
                    x: tilex,
                    y: tiley,
                    layer: 0,
                    id: 1,
                })
            } else {
                let name: String = format!("{}-{}", texture.name(), id);
                let allocation = atlas.upload(
                    name,
                    image.as_bytes(),
                    tilesize,
                    tilesize,
                    0,
                    renderer,
                )?;
                let (posx, posy) = allocation.position();
                tilesheet.tiles.push(Tile {
                    x: tilex,
                    y: tiley,
                    layer: allocation.layer,
                    id: (posx / tilesize) + ((posy / tilesize) * atlas_width),
                })
            }
        }

        // We return as Some(tilesheet) this allows us to check above upon
        // upload if a tile failed to get added or not due to no more room.
        Some(tilesheet)
    }

    pub fn upload(
        texture: Texture,
        renderer: &GpuRenderer,
        atlas: &mut AtlasGroup,
        tilesize: u32,
    ) -> Option<()> {
        let tilecount =
            (texture.size().0 / tilesize) * (texture.size().1 / tilesize);
        let sheet_width = texture.size().0 / tilesize;
        let sheet_height = texture.size().1 / tilesize;
        let bytes = texture.bytes();

        // lets check this to add in the empty tile set first if nothing else yet exists.
        // Also lets add the black tile.
        if atlas.atlas.cache.len() == 0 {
            let image: RgbaImage = ImageBuffer::new(tilesize, tilesize);
            atlas.upload(
                "Empty".to_owned(),
                image.as_bytes(),
                tilesize,
                tilesize,
                0,
                renderer,
            )?;

            let mut image: RgbaImage = ImageBuffer::new(tilesize, tilesize);

            for (_x, _y, pixel) in image.enumerate_pixels_mut() {
                *pixel = image::Rgba([0, 0, 0, 255]);
            }

            atlas.upload(
                "black".to_owned(),
                image.as_bytes(),
                tilesize,
                tilesize,
                0,
                renderer,
            )?;
        }

        for id in 0..tilecount {
            let mut image: RgbaImage = ImageBuffer::new(tilesize, tilesize);

            // lets create the tile from the texture.
            for y in 0..tilesize {
                for x in 0..tilesize {
                    let pos = (((id % sheet_width) * tilesize + (x * 4))
                        * ((id / sheet_height) * tilesize + y))
                        as usize;
                    let pixel = image::Rgba::<u8>([
                        bytes[pos],
                        bytes[pos + 1],
                        bytes[pos + 2],
                        bytes[pos + 3],
                    ]);
                    image.put_pixel(x, y, pixel);
                }
            }

            let name: String = format!("{}-{}", texture.name(), id);
            atlas.upload(
                name,
                image.as_bytes(),
                tilesize,
                tilesize,
                0,
                renderer,
            )?;
        }

        // We return as Some(()) this allows us to check above upon
        // upload if a tile failed to get added or not due to no more room.
        Some(())
    }
}
