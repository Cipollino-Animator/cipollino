use crate::{editor::{state::EditorState, toasts::Toasts}, project::{file::{FileList, FilePtr}, Project}};

use std::{collections::HashSet, fs, path::PathBuf};

use bson::bson;

use crate::{project::{graphic::Graphic, obj::ObjBox}, util::{bson::u64_to_bson, fs::read_json_file}};

use super::asset_file::AssetFile;

use super::super::{file::{audio::AudioFile, FileType}, folder::Folder, obj::{asset::Asset, ObjPtr, ObjSerialize}, palette::Palette};

pub struct LoadingMetadata {
    pub audio_file_ptrs: HashSet<FilePtr<AudioFile>> 
}

impl LoadingMetadata {

    pub fn new() -> Self {
        Self {
            audio_file_ptrs: HashSet::new() 
        }
    }

    fn display_file_missing_errors<T: FileType>(&self, state: &mut EditorState, toasts: &mut Toasts) {
        for (path, key) in T::get_list(&state.project).path_lookup.iter() {
            if let None = T::get_list(&state.project).get(&FilePtr::from_key(*key)) {
                if T::list_in_loading_metadata(self).contains(&FilePtr::from_key(*key)) {
                    toasts.error_toast(format!("File '{}' missing.", path.to_str().unwrap()));
                }
            }
        } 
    }

    pub fn display_errors(&self, state: &mut EditorState, toasts: &mut Toasts) {
        self.display_file_missing_errors::<AudioFile>(state, toasts)
    }

}

impl Project {

    pub fn load(proj_file_path: PathBuf) -> (Self, LoadingMetadata) {
        let mut metadata = LoadingMetadata::new();

        let mut res = if let Some(proj_data) = read_json_file(&proj_file_path) {
            let mut fps = 24.0;
            let mut sample_rate = 44100.0;
            if let Some(new_fps) = proj_data.get("fps").map_or(None, |val| val.as_f64()) {
                fps = new_fps as f32;
            }
            if let Some(new_sample_rate) = proj_data.get("sample_rate").map_or(None, |val| val.as_f64()) {
                sample_rate = new_sample_rate as f32;
            }

            let mut res = Self::new(proj_file_path.clone(), fps, sample_rate);
            if let Some(audio_file_lookups) = proj_data.get("audio_files") {
                res.audio_files.load_lookups(audio_file_lookups.clone());       
            }

            res
        } else {
            Self::new(proj_file_path.clone(), 24.0, 44100.0)
        };

        let folder_path = proj_file_path.parent().unwrap();
        res.root_folder = res.load_folder(&folder_path.to_owned(), ObjPtr::null(), &mut metadata); 

        (res, metadata)
    }

    fn load_folder(&mut self, path: &PathBuf, parent: ObjPtr<Folder>, metadata: &mut LoadingMetadata) -> ObjBox<Folder> {
        let res = self.folders.add(Folder::new(parent));
        res.get_mut(self).name = path.file_name().unwrap().to_str().unwrap().to_owned();

        if let Ok(paths) = fs::read_dir(path) {
            for path in paths {
                if let Ok(path) = path {
                    let path = path.path();
                    self.load_file(path, res.make_ptr(), metadata);
                }
            }
        }
        res
    } 

    fn load_asset<T: Asset + ObjSerialize>(&mut self, path: PathBuf, folder: ObjPtr<Folder>, metadata: &mut LoadingMetadata) -> Option<()> {
        let mut asset_file = AssetFile::open(path.clone()).ok()?;

        let root_obj_ptr = if T::get_list(self).get(ObjPtr::from_key(asset_file.root_obj_key)).is_some() {
            T::get_list_mut(self).next_ptr()
        } else {
            ObjPtr::from_key(asset_file.root_obj_key)
        };
        asset_file.set_root_obj_key(root_obj_ptr.key).ok()?;
        
        T::get_list_mut(self).obj_file_ptrs.borrow_mut().insert(root_obj_ptr, asset_file.root_obj_ptr);
        let obj_box = ObjBox::<T>::obj_deserialize(self, &bson!({
            "key": u64_to_bson(asset_file.root_obj_key),
            "ptr": u64_to_bson(asset_file.root_obj_ptr) 
        }), folder.into(), &mut asset_file, metadata)?;

        *obj_box.get_mut(self).name_mut() = path.file_stem().unwrap().to_str().unwrap().to_owned();
        T::get_list_in_parent_mut(self, folder)?.push(obj_box);

        Some(())
    }

    fn load_file_asset<T: FileType>(&mut self, path: PathBuf, folder: ObjPtr<Folder>) -> Option<()> {
        let base_path = self.base_path();
        let file = FileList::<T>::load_file(self, base_path, path, folder)?;
        T::list_in_folder_mut(self.folders.get_mut(folder)?).push(file);
        Some(())
    }

    fn load_file(&mut self, path: PathBuf, folder_ptr: ObjPtr<Folder>, metadata: &mut LoadingMetadata) { 
        if self.folders.get_mut(folder_ptr).is_none() {
            return;
        }

        if let Some(ext) = path.extension() {
            match ext.to_str().unwrap() {
                "cipgfx" => {
                    let _ = self.load_asset::<Graphic>(path.clone(), folder_ptr, metadata);
                },
                "cippal" => {
                    let _ = self.load_asset::<Palette>(path.clone(), folder_ptr, metadata);
                },
                "mp3" => {
                    let _ = self.load_file_asset::<AudioFile>(path.clone(), folder_ptr);
                },
                _ => {}
            }
        } 
        if path.is_dir() {
            let sub_folder = self.load_folder(&path, folder_ptr, metadata);
            let folder = self.folders.get_mut(folder_ptr).unwrap();
            folder.folders.push(sub_folder);
        }
    }

    pub fn load_file_to_root_folder(&mut self, path: PathBuf, metadata: &mut LoadingMetadata) {
        self.load_file(path, self.root_folder.make_ptr(), metadata);
    }

    pub fn base_path(&self) -> PathBuf {
        self.save_path.parent().unwrap().to_owned()
    }

}
