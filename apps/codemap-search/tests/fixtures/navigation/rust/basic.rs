use crate::prelude::*;
use crate::user::UserService as Service;

pub struct UserService;

impl UserService {
    pub fn save(&self) {}
}

pub fn run() {
    let service: Service = Service::new();
    service.save();
    UserService::save(&service);
}
