use crate::stack::StackConfig;

#[derive(Debug)]
pub struct Yonga {
    pub config: StackConfig,
}

impl Yonga {
    pub fn new(config: StackConfig) -> Self {
        Yonga { config }
    }
}


#[derive(Debug)]
pub struct Spread{
    pub config: StackConfig,
}

impl Spread {
    pub fn new(config: StackConfig) -> Self {
        Spread { config }
    }
}

#[derive(Debug)]
pub struct Binpack{
    pub config: StackConfig,
}

impl Binpack {
    pub fn new(config: StackConfig) -> Self {
        Binpack { config }
    }
}

#[derive(Debug)]
pub struct Random{
    pub config: StackConfig,
}

impl Random {
    pub fn new(config: StackConfig) -> Self {
        Random { config }
    }
}