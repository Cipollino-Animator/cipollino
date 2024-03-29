
use std::{fs, path::PathBuf};

use serde_json::json;

use crate::{project::{graphic::Graphic, obj::ObjBox}, util::fs::{read_bson_file, read_json_file, write_bson_file, write_json_file}};
use sha2::Digest;

use super::{file::{audio::AudioFile, FilePtr, FileType}, folder::Folder, obj::{asset::Asset, ObjPtr, ObjSerialize}, palette::Palette, Project};



impl Project {

    pub fn save(&mut self) {
        self.save_folder(self.root_folder.make_ptr(), &self.save_path.clone());
        
        write_json_file(&self.save_path.clone().with_file_name("proj.cip"), json!({
            "fps": self.fps,
            "sample_rate": self.sample_rate
        }));

        self.save_path = self.save_path.clone().with_file_name("proj.cip");
    }

    pub fn save_folder(&mut self, folder: ObjPtr<Folder>, path: &PathBuf) {
        let mut subfolders = Vec::new();
        if let Some(folder) = self.folders.get(folder) {
            for subfolder in &folder.folders {
                let mut new_path = path.with_file_name(subfolder.get(self).name.clone()); 
                let _ = std::fs::create_dir(new_path.clone());
                new_path.push("a");
                subfolders.push((subfolder.make_ptr(), new_path));
            }
            for gfx_box in &folder.graphics {
                let gfx = gfx_box.get(self);
                let data = gfx_box.obj_serialize(self);
                write_bson_file(&path.with_file_name(format!("{}.{}", gfx.name, gfx.extension())), data);
            }
            for palette_box in &folder.palettes {
                let palette = palette_box.get(self);
                let data = palette_box.obj_serialize(self);
                write_bson_file(&path.with_file_name(format!("{}.{}", palette.name(), palette.extension())), data);
            }
        }
        for (ptr, path) in subfolders {
            self.save_folder(ptr, &path);
        }
    }

    pub fn load(proj_file_path: PathBuf) -> Self {
        let mut fps = 24.0;
        let mut sample_rate = 44100.0;
        if let Some(proj_data) = read_json_file(&proj_file_path) {
            if let Some(new_fps) = proj_data.get("fps").map_or(None, |val| val.as_f64()) {
                fps = new_fps as f32;
            }
            if let Some(new_sample_rate) = proj_data.get("sample_rate").map_or(None, |val| val.as_f64()) {
                sample_rate = new_sample_rate as f32;
            }
        }

        let mut res = Self::new(proj_file_path.clone(), fps, sample_rate);

        let mut base_folder_path = proj_file_path.clone();
        base_folder_path.pop();

        let folder_path = proj_file_path.parent().unwrap();
        res.root_folder = res.load_folder(&folder_path.to_owned(), ObjPtr::null()); 

        res
    }

    fn load_folder(&mut self, path: &PathBuf, parent: ObjPtr<Folder>) -> ObjBox<Folder> {
        let res = self.folders.add(Folder::new(parent));
        res.get_mut(self).name = path.file_name().unwrap().to_str().unwrap().to_owned();

        if let Ok(paths) = fs::read_dir(path) {
            for path in paths {
                if let Ok(path) = path {
                    let path = path.path();
                    self.load_file(path, res.make_ptr());
                }
            }
        }
        res
    } 

    fn load_file(&mut self, path: PathBuf, folder_ptr: ObjPtr<Folder>) { 
        if self.folders.get_mut(folder_ptr).is_none() {
            return;
        }

        if let Some(ext) = path.extension() {
            match ext.to_str().unwrap() {
                "cipgfx" => {
                    if let Some(data) = read_bson_file(&path) { 
                        if let Some(gfx) = ObjBox::<Graphic>::obj_deserialize(self, &data, folder_ptr.into()) {
                            gfx.get_mut(self).name = path.file_stem().unwrap().to_str().unwrap().to_owned();
                            let folder = self.folders.get_mut(folder_ptr).unwrap();
                            folder.graphics.push(gfx);
                        }
                    }
                },
                "cippal" => {
                    if let Some(data) = read_bson_file(&path) { 
                        if let Some(palette) = ObjBox::<Palette>::obj_deserialize(self, &data, folder_ptr.into()) {
                            let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
                            palette.get_mut(self).name = name; 
                            let folder = self.folders.get_mut(folder_ptr).unwrap();
                            folder.palettes.push(palette);
                        }
                    }
                },
                "mp3" => {
                    let file_ptr = self.get_file_ptr(&path, &self.base_path());
                    let data = AudioFile::load(&path);
                    if let Some(file_ptr) = file_ptr {
                        if let Some(data) = data {
                            let folder = self.folders.get_mut(folder_ptr).unwrap();
                            folder.audios.push(file_ptr.clone());
                            self.audio_files.insert(file_ptr, data);
                        }
                    }
                },
                _ => {}
            }
        } 
        if path.is_dir() {
            let sub_folder = self.load_folder(&path, folder_ptr);
            let folder = self.folders.get_mut(folder_ptr).unwrap();
            folder.folders.push(sub_folder);
        }
    }

    pub fn load_file_to_root_folder(&mut self, path: PathBuf) {
        self.load_file(path, self.root_folder.make_ptr());
    }

    pub fn get_file_ptr<T: FileType>(&mut self, path: &PathBuf, base: &PathBuf) -> Option<FilePtr<T>> {
        let rel_path = pathdiff::diff_paths(path, base)?; 
        let mut hash = sha2::Sha256::new();
        hash.update(fs::read(path).ok()?);
        let hash = hash.finalize();
        let mut hash_val: u64 = 0;
        for i in 0..8 {
            hash_val <<= 8;
            hash_val |= hash[i] as u64;
        }

        let file_ptr = FilePtr::<T>::new(rel_path.clone(), hash_val); 
    
        self.path_file_ptr.insert(rel_path, file_ptr.ptr.clone());
        self.hash_file_ptr.insert(hash_val, file_ptr.ptr.clone());

        Some(file_ptr)
    }

    pub fn base_path(&self) -> PathBuf {
        self.save_path.parent().unwrap().to_owned()
    }

}
