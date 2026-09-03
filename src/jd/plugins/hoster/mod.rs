use std::rc::Rc;

pub mod onefichiercom;

pub trait HosterAccountBuilder {
    fn widget(&self) -> gtk4::Box;
    fn credentials(&self) -> (String, String);
}

pub fn builder_for(hoster: &str) -> Option<Rc<dyn HosterAccountBuilder>> {
    let h = hoster.to_lowercase();
    if h.contains("1fichier") {
        Some(Rc::new(onefichiercom::OneFichierCom::new()))
    } else {
        None
    }
}
