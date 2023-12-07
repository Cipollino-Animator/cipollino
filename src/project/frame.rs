
use super::{Project, action::ObjAction, ObjData};

#[derive(Clone)]
pub struct FrameData {
    pub time: i32,
    pub layer: u64
}

impl ObjData for FrameData {

    fn add(&self, key: u64, project: &mut Project) {
        project.add_frame_with_key(key, self.layer, self.time);
    }

    fn delete(&self, key: u64, project: &mut Project) {
        project.delete_frame(key);
    }

    fn set(&self, key: u64, project: &mut Project) {
        project.set_frame_data(key, self.clone());
    }

}

pub struct Frame {
    pub data: FrameData,
    pub strokes: Vec<u64>
}

impl Project {

    pub fn add_frame(&mut self, layer: u64, time: i32) -> Option<(u64, ObjAction)> {
        let key = self.next_key();
        self.add_frame_with_key(key, layer, time)
    }
    
    pub fn add_frame_with_key(&mut self, key: u64, layer: u64, time: i32) -> Option<(u64, ObjAction)> {
        self.layers.get_mut(&layer)?.frames.push(key);
        let data = FrameData {
            layer,
            time 
        };
        self.frames.insert(key, Frame {
            data: data.clone(), 
            strokes: Vec::new()
        });
        Some((key, ObjAction::addition(key, data)))
    } 

    pub fn delete_frame(&mut self, key: u64) -> Option<Vec<ObjAction>> {
        let frame = self.frames.remove(&key)?;
        let mut acts = Vec::new();
        for stroke in frame.strokes {
            if let Some(mut stroke_acts) = self.delete_stroke(stroke) {
                acts.append(&mut stroke_acts);
            }
        }
        if let Some(layer) = self.layers.get_mut(&frame.data.layer) {
            layer.frames.retain(|frame| *frame != key);
        }
        acts.push(ObjAction::deletion(key, frame.data));
        Some(acts) 
    }
    
    pub fn set_frame_data(&mut self, key: u64, data: FrameData) -> Option<ObjAction> {
        let frame = self.frames.get_mut(&key)?;
        let res = ObjAction::modification(key, frame.data.clone(), data.clone());
        frame.data = data;
        Some(res)
    }

}
