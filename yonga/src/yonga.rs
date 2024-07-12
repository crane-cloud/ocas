use crate::stack::StackConfig;
use crate::utility::Config;
use crate::solver::Solver;

#[derive(Debug)]
pub struct Yonga {
    pub stack_config: StackConfig,
    pub cluster_config: Config,
    pub running: bool,
    pub revision: u32,
    pub solver: Solver,
}

impl Yonga {
    pub fn new(stack_config: StackConfig, cluster_config: Config, solver: Solver) -> Self {
        Yonga { 
            stack_config,
            cluster_config,
            running: false,
            revision: 0,
            solver,
        }
    }

    pub async fn start(&mut self) {
        println!("Starting Yonga placement strategy");

        loop { // consider the different modes of placement (0, 1, 2)
            if self.running {
                self.placement_n().await;
            }

            else {
                self.placement_0().await;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }
    }

    pub async fn placement_0(&mut self) {
        println!("Running the Yonga placement strategy - placement 0");

        let _ = self.solver.solve_0().await;

        // update the run and revision
        self.running = true;
        self.revision += 1;

    }

    pub async fn placement_n(&mut self) {
        println!("Evaluating the current state of the placements");
    }

}