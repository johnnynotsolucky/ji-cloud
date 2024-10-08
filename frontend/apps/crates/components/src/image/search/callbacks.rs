use shared::domain::module::body::Image;

pub struct Callbacks {
    pub on_select: Option<Box<dyn Fn(Option<Image>)>>,
}

impl Callbacks {
    pub fn new(on_select: Option<impl Fn(Option<Image>) + 'static>) -> Self {
        Self {
            on_select: on_select.map(|f| Box::new(f) as _),
        }
    }
}
