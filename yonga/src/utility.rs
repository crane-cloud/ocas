pub struct Microservice {
    pub name: String,
    pub replicas: u8,
    pub node: Option<String>,
    pub status: bool,
}

pub fn microservices () -> Vec<Microservice> {

}