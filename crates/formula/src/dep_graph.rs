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

    /// Like [`get_recalc_order`], but on a cycle returns the partial order plus
    /// the exact cells stuck in cycles instead of failing the whole sheet.
    /// Excel-like semantics: only loop members evaluate to #CYCLE!, the rest
    /// of the sheet still recalculates.
    pub fn get_recalc_order_with_cycles(
        &self,
        start_cells: &[CellCoord],
    ) -> (Vec<CellCoord>, Vec<CellCoord>) {
        let mut in_degree: HashMap<CellCoord, usize> = HashMap::new();
        let mut affected: HashSet<CellCoord> = HashSet::new();
        let mut queue: VecDeque<CellCoord> = VecDeque::new();

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

        if order.len() == affected.len() {
            return (order, Vec::new());
        }
        // Remaining cells with nonzero in-degree are exactly the cells inside
        // (or downstream of) a cycle. Cells still reachable from a cycle but
        // not part of one are also stuck — Excel shows #CYCLE-like errors for
        // them too, so keeping them in the poisoned set matches expectations.
        let poisoned: Vec<CellCoord> = affected
            .iter()
            .copied()
            .filter(|cell| in_degree.get(cell).is_some_and(|deg| *deg > 0))
            .collect();
        (order, poisoned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn detects_dependency_cycle() {
        let mut graph = DependencyGraph::default();
        graph.update_cell_deps((1, 1), HashSet::from([(2, 1)]));
        graph.update_cell_deps((2, 1), HashSet::from([(1, 1)]));

        let result = graph.get_recalc_order(&[(1, 1)]);
        assert_eq!(result, Err(FormulaError::Cycle));
    }

    #[test]
    fn returns_topological_order_for_acyclic_graph() {
        let mut graph = DependencyGraph::default();
        graph.update_cell_deps((1, 1), HashSet::new());
        graph.update_cell_deps((2, 1), HashSet::from([(1, 1)]));
        graph.update_cell_deps((3, 1), HashSet::from([(2, 1)]));

        let order = graph
            .get_recalc_order(&[(1, 1)])
            .expect("acyclic graph should order");
        assert_eq!(order.first(), Some(&(1, 1)));
        assert!(order.contains(&(3, 1)));
    }

    #[test]
    fn cycle_isolation_poisons_only_loop_members() {
        // A1 = B1, B1 = A1 (cycle), C1 = A1 (innocent downstream of A1's
        // own value), D1 = 5 (untouched independent formula).
        let mut graph = DependencyGraph::default();
        graph.update_cell_deps((1, 1), HashSet::from([(2, 1)]));
        graph.update_cell_deps((2, 1), HashSet::from([(1, 1)]));
        graph.update_cell_deps((3, 1), HashSet::from([(4, 4)]));
        graph.update_cell_deps((4, 4), HashSet::new());

        // Start from all formulas: cycle in {A1,B1}, but C1/D1 still order.
        let (order, poisoned) =
            graph.get_recalc_order_with_cycles(&[(1, 1), (2, 1), (3, 1), (4, 4)]);
        assert!(poisoned.contains(&(1, 1)));
        assert!(poisoned.contains(&(2, 1)));
        assert!(!poisoned.contains(&(3, 1)));
        assert!(!poisoned.contains(&(4, 4)));
        assert!(order.contains(&(3, 1)));
        assert!(order.contains(&(4, 4)));
    }
}
