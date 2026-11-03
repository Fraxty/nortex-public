//! Conservative MIR folding and liveness-based dead-code elimination.

use std::collections::{HashSet, VecDeque};

use crate::mir::{
    BasicBlockId, BinaryOp, LocalId, MirBody, Operand, Place, Rvalue, StatementKind, TerminatorKind,
};

pub fn optimize(body: &mut MirBody) {
    for block in &mut body.basic_blocks {
        for statement in &mut block.statements {
            let folded = match &statement.kind {
                StatementKind::Assign(place, Rvalue::BinaryOp(operator, left, right)) => {
                    fold_binary(*operator, left, right).map(|value| (place.clone(), value))
                }
                _ => None,
            };
            if let Some((place, value)) = folded {
                statement.kind = StatementKind::Assign(place, Rvalue::Use(value));
            }
        }

        if let Some(terminator) = &mut block.terminator {
            let target = match &terminator.kind {
                TerminatorKind::SwitchInt {
                    discr: Operand::ConstantInt(value),
                    targets,
                    otherwise,
                } => Some(
                    targets
                        .iter()
                        .find(|(candidate, _)| candidate == value)
                        .map_or(*otherwise, |(_, target)| *target),
                ),
                _ => None,
            };
            if let Some(target) = target {
                terminator.kind = TerminatorKind::Goto { target };
            }
        }
    }
    eliminate_dead_code(body);
}

fn eliminate_dead_code(body: &mut MirBody) {
    loop {
        let Some(cfg_successors) = successors(body) else {
            return;
        };
        let reachable = reachable_blocks(&cfg_successors);
        let mut changed = remove_unreachable_blocks(body, &reachable);
        let Some(cfg_successors) = successors(body) else {
            return;
        };
        let live_out = live_out_sets(body, &cfg_successors);

        for (block_index, block) in body.basic_blocks.iter_mut().enumerate() {
            let mut live = live_out[block_index].clone();
            if let Some(terminator) = &block.terminator {
                add_terminator_uses(&terminator.kind, &mut live);
            }
            let mut retained = Vec::with_capacity(block.statements.len());
            for statement in block.statements.drain(..).rev() {
                if let StatementKind::Assign(place, value) = &statement.kind {
                    if place.projection.is_empty()
                        && !live.contains(&place.local)
                        && is_removable_rvalue(value)
                    {
                        changed = true;
                        continue;
                    }
                }
                update_live_set(&statement.kind, &mut live);
                retained.push(statement);
            }
            retained.reverse();
            block.statements = retained;
        }

        if !changed {
            break;
        }
    }
}

fn successors(body: &MirBody) -> Option<Vec<Vec<BasicBlockId>>> {
    let block_count = body.basic_blocks.len();
    body.basic_blocks
        .iter()
        .map(|block| {
            let mut targets = Vec::new();
            if let Some(terminator) = &block.terminator {
                match &terminator.kind {
                    TerminatorKind::Goto { target } => targets.push(*target),
                    TerminatorKind::SwitchInt {
                        targets: cases,
                        otherwise,
                        ..
                    } => {
                        targets.extend(cases.iter().map(|(_, target)| *target));
                        targets.push(*otherwise);
                    }
                    TerminatorKind::Drop { target, unwind, .. } => {
                        targets.push(*target);
                        targets.extend(unwind.iter().copied());
                    }
                    TerminatorKind::Call {
                        target, cleanup, ..
                    }
                    | TerminatorKind::Assert {
                        target, cleanup, ..
                    } => {
                        targets.push(*target);
                        targets.extend(cleanup.iter().copied());
                    }
                    TerminatorKind::Return | TerminatorKind::Unreachable => {}
                }
            }
            targets
                .iter()
                .all(|target| target.0 < block_count)
                .then_some(targets)
        })
        .collect()
}

fn reachable_blocks(successors: &[Vec<BasicBlockId>]) -> HashSet<usize> {
    let mut reachable = HashSet::new();
    let mut pending = VecDeque::from([0usize]);
    while let Some(block) = pending.pop_front() {
        if !reachable.insert(block) {
            continue;
        }
        pending.extend(successors[block].iter().map(|target| target.0));
    }
    reachable
}

fn remove_unreachable_blocks(body: &mut MirBody, reachable: &HashSet<usize>) -> bool {
    if body.basic_blocks.is_empty() || reachable.len() == body.basic_blocks.len() {
        return false;
    }
    let mut remap = vec![None; body.basic_blocks.len()];
    let mut retained = Vec::with_capacity(reachable.len());
    for (old_index, block) in body.basic_blocks.drain(..).enumerate() {
        if reachable.contains(&old_index) {
            remap[old_index] = Some(BasicBlockId(retained.len()));
            retained.push(block);
        }
    }
    for block in &mut retained {
        if let Some(terminator) = &mut block.terminator {
            remap_terminator_targets(&mut terminator.kind, &remap);
        }
    }
    body.basic_blocks = retained;
    true
}

fn remap_terminator_targets(kind: &mut TerminatorKind, remap: &[Option<BasicBlockId>]) {
    let remap_target = |target: &mut BasicBlockId| {
        if let Some(mapped) = remap.get(target.0).copied().flatten() {
            *target = mapped;
        }
    };
    match kind {
        TerminatorKind::Goto { target } => remap_target(target),
        TerminatorKind::SwitchInt {
            targets, otherwise, ..
        } => {
            for (_, target) in targets {
                remap_target(target);
            }
            remap_target(otherwise);
        }
        TerminatorKind::Drop { target, unwind, .. } => {
            remap_target(target);
            if let Some(unwind) = unwind {
                remap_target(unwind);
            }
        }
        TerminatorKind::Call {
            target, cleanup, ..
        }
        | TerminatorKind::Assert {
            target, cleanup, ..
        } => {
            remap_target(target);
            if let Some(cleanup) = cleanup {
                remap_target(cleanup);
            }
        }
        TerminatorKind::Return | TerminatorKind::Unreachable => {}
    }
}

fn live_out_sets(body: &MirBody, successors: &[Vec<BasicBlockId>]) -> Vec<HashSet<LocalId>> {
    let mut live_in = vec![HashSet::new(); body.basic_blocks.len()];
    let mut live_out = live_in.clone();
    loop {
        let mut changed = false;
        for index in (0..body.basic_blocks.len()).rev() {
            let mut out = HashSet::new();
            for successor in &successors[index] {
                out.extend(live_in[successor.0].iter().copied());
            }
            let mut input = out.clone();
            let block = &body.basic_blocks[index];
            if let Some(terminator) = &block.terminator {
                add_terminator_uses(&terminator.kind, &mut input);
            }
            for statement in block.statements.iter().rev() {
                update_live_set(&statement.kind, &mut input);
            }
            if out != live_out[index] || input != live_in[index] {
                live_out[index] = out;
                live_in[index] = input;
                changed = true;
            }
        }
        if !changed {
            return live_out;
        }
    }
}

fn update_live_set(statement: &StatementKind, live: &mut HashSet<LocalId>) {
    match statement {
        StatementKind::Assign(place, value) => {
            if place.projection.is_empty() {
                live.remove(&place.local);
            } else {
                add_place_uses(place, live);
            }
            add_rvalue_uses(value, live);
        }
        StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
            live.remove(local);
        }
        StatementKind::Deinit(place) => add_place_uses(place, live),
        StatementKind::Nop => {}
    }
}

fn add_terminator_uses(kind: &TerminatorKind, live: &mut HashSet<LocalId>) {
    match kind {
        TerminatorKind::SwitchInt { discr, .. } => add_operand_uses(discr, live),
        TerminatorKind::Drop { place, .. } => add_place_uses(place, live),
        TerminatorKind::Call {
            args, destination, ..
        } => {
            for argument in args {
                add_operand_uses(argument, live);
            }
            if destination.projection.is_empty() {
                live.remove(&destination.local);
            } else {
                add_place_uses(destination, live);
            }
        }
        TerminatorKind::Assert { cond, .. } => add_operand_uses(cond, live),
        TerminatorKind::Return => {
            live.insert(LocalId(0));
        }
        TerminatorKind::Goto { .. } | TerminatorKind::Unreachable => {}
    }
}

fn add_rvalue_uses(value: &Rvalue, live: &mut HashSet<LocalId>) {
    match value {
        Rvalue::Use(operand) | Rvalue::Cast(operand, _) => add_operand_uses(operand, live),
        Rvalue::Ref(_, place) | Rvalue::Discriminant(place) => add_place_uses(place, live),
        Rvalue::BinaryOp(_, left, right) => {
            add_operand_uses(left, live);
            add_operand_uses(right, live);
        }
        Rvalue::Aggregate(operands) => {
            for operand in operands {
                add_operand_uses(operand, live);
            }
        }
    }
}

fn add_operand_uses(operand: &Operand, live: &mut HashSet<LocalId>) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => add_place_uses(place, live),
        Operand::ConstantInt(_)
        | Operand::ConstantFloat(_)
        | Operand::ConstantBool(_)
        | Operand::ConstantString(_) => {}
    }
}

fn add_place_uses(place: &Place, live: &mut HashSet<LocalId>) {
    live.insert(place.local);
    for projection in &place.projection {
        if let crate::mir::ProjectionElem::Index(local) = projection {
            live.insert(*local);
        }
    }
}

fn is_removable_rvalue(value: &Rvalue) -> bool {
    fn operand_is_non_moving(operand: &Operand) -> bool {
        !matches!(operand, Operand::Move(_))
    }
    match value {
        Rvalue::Use(operand) => operand_is_non_moving(operand),
        Rvalue::BinaryOp(
            BinaryOp::BitXor
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::Eq
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Ne
            | BinaryOp::Ge
            | BinaryOp::Gt,
            left,
            right,
        ) => operand_is_non_moving(left) && operand_is_non_moving(right),
        Rvalue::Cast(_, _)
        | Rvalue::BinaryOp(_, _, _)
        | Rvalue::Aggregate(_)
        | Rvalue::Discriminant(_)
        | Rvalue::Ref(_, _) => false,
    }
}

fn fold_binary(operator: BinaryOp, left: &Operand, right: &Operand) -> Option<Operand> {
    match (left, right) {
        (Operand::ConstantInt(left), Operand::ConstantInt(right)) => {
            let value = match operator {
                BinaryOp::Add => Operand::ConstantInt(left.checked_add(*right)?),
                BinaryOp::Sub => Operand::ConstantInt(left.checked_sub(*right)?),
                BinaryOp::Mul => Operand::ConstantInt(left.checked_mul(*right)?),
                BinaryOp::Div => Operand::ConstantInt(left.checked_div(*right)?),
                BinaryOp::Rem => Operand::ConstantInt(left.checked_rem(*right)?),
                BinaryOp::BitXor => Operand::ConstantInt(left ^ right),
                BinaryOp::BitAnd => Operand::ConstantInt(left & right),
                BinaryOp::BitOr => Operand::ConstantInt(left | right),
                BinaryOp::Shl => {
                    let shift = u32::try_from(*right).ok()?;
                    let shifted = left.checked_shl(shift)?;
                    if shift > 0 && shifted >> shift != *left {
                        return None;
                    }
                    Operand::ConstantInt(shifted)
                }
                BinaryOp::Shr => {
                    let shift = u32::try_from(*right).ok()?;
                    Operand::ConstantInt(left.checked_shr(shift)?)
                }
                BinaryOp::Eq => Operand::ConstantBool(left == right),
                BinaryOp::Ne => Operand::ConstantBool(left != right),
                BinaryOp::Lt => Operand::ConstantBool(left < right),
                BinaryOp::Le => Operand::ConstantBool(left <= right),
                BinaryOp::Ge => Operand::ConstantBool(left >= right),
                BinaryOp::Gt => Operand::ConstantBool(left > right),
            };
            Some(value)
        }
        (Operand::ConstantBool(left), Operand::ConstantBool(right)) => match operator {
            BinaryOp::Eq => Some(Operand::ConstantBool(left == right)),
            BinaryOp::Ne => Some(Operand::ConstantBool(left != right)),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{BasicBlockData, BasicBlockId, Place, Statement, Terminator};

    fn assignment(operator: BinaryOp, left: Operand, right: Operand) -> Statement {
        assignment_to(1, operator, left, right)
    }

    fn assignment_to(local: usize, operator: BinaryOp, left: Operand, right: Operand) -> Statement {
        Statement {
            kind: StatementKind::Assign(
                Place::from_local(crate::mir::LocalId(local)),
                Rvalue::BinaryOp(operator, left, right),
            ),
            source_span: (0, 1),
        }
    }

    #[test]
    fn folds_integer_and_boolean_constants() {
        let mut body = MirBody::new(0);
        body.basic_blocks.push(BasicBlockData {
            statements: vec![
                assignment_to(
                    1,
                    BinaryOp::Add,
                    Operand::ConstantInt(20),
                    Operand::ConstantInt(22),
                ),
                assignment_to(
                    2,
                    BinaryOp::Lt,
                    Operand::ConstantInt(1),
                    Operand::ConstantInt(2),
                ),
                assignment_to(
                    3,
                    BinaryOp::Eq,
                    Operand::ConstantBool(true),
                    Operand::ConstantBool(false),
                ),
            ],
            terminator: Some(Terminator {
                kind: TerminatorKind::Call {
                    callee: "use_values".into(),
                    args: vec![
                        Operand::Copy(Place::from_local(crate::mir::LocalId(1))),
                        Operand::Copy(Place::from_local(crate::mir::LocalId(2))),
                        Operand::Copy(Place::from_local(crate::mir::LocalId(3))),
                    ],
                    destination: Place::from_local(crate::mir::LocalId(0)),
                    target: BasicBlockId(1),
                    cleanup: None,
                },
                source_span: (0, 1),
            }),
            ..BasicBlockData::default()
        });
        body.basic_blocks.push(BasicBlockData {
            terminator: Some(Terminator {
                kind: TerminatorKind::Return,
                source_span: (1, 2),
            }),
            ..BasicBlockData::default()
        });

        optimize(&mut body);

        let statements = &body.basic_blocks[0].statements;
        assert!(matches!(
            statements[0].kind,
            StatementKind::Assign(_, Rvalue::Use(Operand::ConstantInt(42)))
        ));
        assert!(matches!(
            statements[1].kind,
            StatementKind::Assign(_, Rvalue::Use(Operand::ConstantBool(true)))
        ));
        assert!(matches!(
            statements[2].kind,
            StatementKind::Assign(_, Rvalue::Use(Operand::ConstantBool(false)))
        ));
    }

    #[test]
    fn preserves_operations_with_overflow_or_undefined_constant_cases() {
        let mut body = MirBody::new(0);
        body.basic_blocks.push(BasicBlockData {
            statements: vec![
                assignment_to(
                    1,
                    BinaryOp::Add,
                    Operand::ConstantInt(i64::MAX),
                    Operand::ConstantInt(1),
                ),
                assignment_to(
                    2,
                    BinaryOp::Div,
                    Operand::ConstantInt(1),
                    Operand::ConstantInt(0),
                ),
                assignment_to(
                    3,
                    BinaryOp::Shl,
                    Operand::ConstantInt(i64::MAX),
                    Operand::ConstantInt(1),
                ),
            ],
            terminator: Some(Terminator {
                kind: TerminatorKind::Call {
                    callee: "use_values".into(),
                    args: vec![
                        Operand::Copy(Place::from_local(crate::mir::LocalId(1))),
                        Operand::Copy(Place::from_local(crate::mir::LocalId(2))),
                        Operand::Copy(Place::from_local(crate::mir::LocalId(3))),
                    ],
                    destination: Place::from_local(crate::mir::LocalId(0)),
                    target: BasicBlockId(1),
                    cleanup: None,
                },
                source_span: (0, 1),
            }),
            ..BasicBlockData::default()
        });
        body.basic_blocks.push(BasicBlockData {
            terminator: Some(Terminator {
                kind: TerminatorKind::Return,
                source_span: (1, 2),
            }),
            ..BasicBlockData::default()
        });

        optimize(&mut body);

        assert!(
            body.basic_blocks[0]
                .statements
                .iter()
                .all(|statement| matches!(
                    statement.kind,
                    StatementKind::Assign(_, Rvalue::BinaryOp(_, _, _))
                ))
        );
    }

    #[test]
    fn resolves_switches_with_constant_discriminants() {
        let mut body = MirBody::new(0);
        body.basic_blocks.push(BasicBlockData {
            terminator: Some(Terminator {
                kind: TerminatorKind::SwitchInt {
                    discr: Operand::ConstantInt(7),
                    targets: vec![(7, BasicBlockId(1))],
                    otherwise: BasicBlockId(2),
                },
                source_span: (0, 1),
            }),
            ..BasicBlockData::default()
        });
        body.basic_blocks.push(BasicBlockData::default());
        body.basic_blocks.push(BasicBlockData::default());

        optimize(&mut body);

        assert!(matches!(
            body.basic_blocks[0]
                .terminator
                .as_ref()
                .map(|term| &term.kind),
            Some(TerminatorKind::Goto {
                target: BasicBlockId(1)
            })
        ));
    }

    #[test]
    fn removes_dead_assignments_and_unreachable_blocks() {
        let mut body = MirBody::new(0);
        body.basic_blocks.push(BasicBlockData {
            statements: vec![
                assignment(
                    BinaryOp::Add,
                    Operand::ConstantInt(2),
                    Operand::ConstantInt(3),
                ),
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(crate::mir::LocalId(2)),
                        Rvalue::Use(Operand::ConstantInt(9)),
                    ),
                    source_span: (1, 2),
                },
            ],
            terminator: Some(Terminator {
                kind: TerminatorKind::SwitchInt {
                    discr: Operand::ConstantInt(1),
                    targets: vec![(1, BasicBlockId(1))],
                    otherwise: BasicBlockId(2),
                },
                source_span: (0, 1),
            }),
            ..BasicBlockData::default()
        });
        body.basic_blocks.push(BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::from_local(crate::mir::LocalId(0)),
                    Rvalue::Use(Operand::ConstantInt(7)),
                ),
                source_span: (2, 3),
            }],
            terminator: Some(Terminator {
                kind: TerminatorKind::Return,
                source_span: (3, 4),
            }),
            ..BasicBlockData::default()
        });
        body.basic_blocks.push(BasicBlockData {
            terminator: Some(Terminator {
                kind: TerminatorKind::Return,
                source_span: (4, 5),
            }),
            ..BasicBlockData::default()
        });

        optimize(&mut body);

        assert_eq!(body.basic_blocks.len(), 2);
        assert!(body.basic_blocks[0].statements.is_empty());
        assert_eq!(body.basic_blocks[1].statements.len(), 1);
        assert!(matches!(
            body.basic_blocks[1].statements[0].kind,
            StatementKind::Assign(_, Rvalue::Use(Operand::ConstantInt(7)))
        ));
    }

    #[test]
    fn dead_code_elimination_preserves_moves_borrows_and_calls() {
        let mut body = MirBody::new(0);
        body.basic_blocks.push(BasicBlockData {
            statements: vec![
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(crate::mir::LocalId(1)),
                        Rvalue::Use(Operand::Move(Place::from_local(crate::mir::LocalId(2)))),
                    ),
                    source_span: (0, 1),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(crate::mir::LocalId(3)),
                        Rvalue::Ref(
                            crate::mir::BorrowKind::Shared,
                            Place::from_local(crate::mir::LocalId(2)),
                        ),
                    ),
                    source_span: (1, 2),
                },
            ],
            terminator: Some(Terminator {
                kind: TerminatorKind::Call {
                    callee: "effect".into(),
                    args: Vec::new(),
                    destination: Place::from_local(crate::mir::LocalId(0)),
                    target: BasicBlockId(0),
                    cleanup: None,
                },
                source_span: (2, 3),
            }),
            ..BasicBlockData::default()
        });

        optimize(&mut body);

        assert_eq!(body.basic_blocks[0].statements.len(), 2);
        assert!(matches!(
            body.basic_blocks[0].terminator.as_ref().map(|term| &term.kind),
            Some(TerminatorKind::Call { callee, .. }) if callee == "effect"
        ));
    }
}
