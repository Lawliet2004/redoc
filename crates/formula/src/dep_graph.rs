use crate::ast::*;
use std::collections::{HashMap, HashSet, VecDeque};

pub type CellCoord = (u32, u32); // (row, col)

#[derive(Default)]
pub struct DependencyGraph {
    pub cell_deps: HashMap<CellCoord, HashSet<CellCoord>>, // Cell -> cells it depends on
    pub dependents: HashMap<CellCoord, HashSet<CellCoord>>, // Cell -> cells that depend on it
}

impl DependencyGraph {
    pub fn update_cell_deps(&mut self, cell: CellCoord, new_deps: HashSet<CellCoord>) {
        // Remove old dependencies
        if let Some(old_deps) = self.cell_deps.remove(&cell) {
            for dep in old_deps {
                if let Some(set) = self.dependents.get_mut(&dep) {
                    set.remove(&cell);
                }
            }
        }

        // Add new dependencies
        for &dep in &new_deps {
            self.dependents.entry(dep).or_default().insert(cell);
        }
        self.cell_deps.insert(cell, new_deps);
    }

    pub fn get_recalc_order(
        &self,
        start_cells: &[CellCoord],
    ) -> Result<Vec<CellCoord>, FormulaError> {
        let mut in_degree: HashMap<CellCoord, usize> = HashMap::new();
        let mut affected: HashSet<CellCoord> = HashSet::new();
        let mut queue: VecDeque<CellCoord> = VecDeque::new();

        // Collect all affected cells downstream
        let mut search = start_cells.to_vec();
        while let Some(cell) = search.pop() {
            if affected.insert(cell) {
                if let Some(deps) = self.dependents.get(&cell) {
                    for &dep in deps {
                        search.push(dep);
                    }
                }
            }
        }

        // Calculate in-degree within the affected subgraph
        for &cell in &affected {
            let deps = self.cell_deps.get(&cell);
            let count = deps.map_or(0, |set| set.iter().filter(|d| affected.contains(d)).count());
            in_degree.insert(cell, count);
            if count == 0 {
                queue.push_back(cell);
            }
        }

        let mut order = Vec::new();
        while let Some(cell) = queue.pop_front() {
            order.push(cell);
            if let Some(deps) = self.dependents.get(&cell) {
                for &dep in deps {
                    if let Some(deg) = in_degree.get_mut(&dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }

        if order.len() < affected.len() {
            // Cycle detected!
            Err(FormulaError::Cycle)
        } else {
            Ok(order)
        }
    }
}
