//! Choice-point stack and backtracking.
//!
//! Per `sw-cor24-prolog/docs/vm-spec.md` §4.1. TRY pushes a frame with
//! the current A0-A7 / CP / HP / TR / EP and the alternative PC.
//! RETRY restores those fields and updates the alt. TRUST pops. FAIL
//! jumps to the top frame's alt (restoration happens when RETRY /
//! TRUST executes).

use super::heap::unwind_trail;

#[derive(Clone, Copy, Debug)]
pub struct ChoicePt {
    pub saved_regs: [u32; 8],
    pub alt_pc: usize,
    pub saved_cp: usize,
    pub saved_hp: usize,
    pub saved_tr: usize,
    pub saved_ep: usize,
}

pub fn push_choice(
    stack: &mut Vec<ChoicePt>,
    regs: &[u32; 16],
    alt_pc: usize,
    cp: usize,
    hp: usize,
    tr: usize,
    ep: usize,
) {
    let mut saved_regs = [0u32; 8];
    saved_regs.copy_from_slice(&regs[..8]);
    stack.push(ChoicePt {
        saved_regs,
        alt_pc,
        saved_cp: cp,
        saved_hp: hp,
        saved_tr: tr,
        saved_ep: ep,
    });
}

pub fn restore_top<F>(
    stack: &[ChoicePt],
    regs: &mut [u32; 16],
    heap: &mut Vec<u32>,
    trail: &mut Vec<u32>,
    mut truncate_env: F,
) -> Option<usize>
where
    F: FnMut(usize),
{
    let top = stack.last()?;
    regs[..8].copy_from_slice(&top.saved_regs);
    unwind_trail(top.saved_tr, heap, trail);
    heap.truncate(top.saved_hp);
    truncate_env(top.saved_ep);
    Some(top.saved_cp)
}

pub fn update_alt(stack: &mut [ChoicePt], new_alt: usize) {
    if let Some(top) = stack.last_mut() {
        top.alt_pc = new_alt;
    }
}

pub fn pop(stack: &mut Vec<ChoicePt>) {
    stack.pop();
}

pub fn top_alt(stack: &[ChoicePt]) -> Option<usize> {
    stack.last().map(|c| c.alt_pc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::heap::{alloc_unbound, bind, deref, is_unbound, make, tag, TAG_ATOM};

    fn noop_env(_: usize) {}

    #[test]
    fn push_and_pop_restores_no_state_if_empty() {
        let mut stack: Vec<ChoicePt> = Vec::new();
        let mut regs = [0u32; 16];
        let mut heap: Vec<u32> = Vec::new();
        let mut trail: Vec<u32> = Vec::new();
        assert!(restore_top(&stack, &mut regs, &mut heap, &mut trail, noop_env).is_none());
        pop(&mut stack);
    }

    #[test]
    fn restore_rewinds_heap_and_trail() {
        let mut stack: Vec<ChoicePt> = Vec::new();
        let mut regs = [0u32; 16];
        let mut heap: Vec<u32> = Vec::new();
        let mut trail: Vec<u32> = Vec::new();
        let _ = alloc_unbound(&mut heap);
        push_choice(&mut stack, &regs, 99, 7, heap.len(), trail.len(), 0);
        let r = alloc_unbound(&mut heap);
        bind(1, make(TAG_ATOM, 5), &mut heap, &mut trail);
        let cp = restore_top(&stack, &mut regs, &mut heap, &mut trail, noop_env).expect("top");
        assert_eq!(cp, 7);
        assert_eq!(heap.len(), 1);
        assert!(trail.is_empty());
        let _ = tag(r);
    }

    #[test]
    fn restore_brings_back_saved_argument_regs() {
        let mut stack: Vec<ChoicePt> = Vec::new();
        let mut regs = [0u32; 16];
        for i in 0..8 {
            regs[i] = make(TAG_ATOM, i as u32 + 1);
        }
        let heap: Vec<u32> = Vec::new();
        let trail: Vec<u32> = Vec::new();
        push_choice(&mut stack, &regs, 0, 0, heap.len(), trail.len(), 0);
        for i in 0..8 {
            regs[i] = 0;
        }
        let mut heap: Vec<u32> = Vec::new();
        let mut trail: Vec<u32> = Vec::new();
        restore_top(&stack, &mut regs, &mut heap, &mut trail, noop_env);
        for i in 0..8 {
            assert_eq!(regs[i], make(TAG_ATOM, i as u32 + 1));
        }
    }

    #[test]
    fn update_alt_changes_top_frame() {
        let mut stack: Vec<ChoicePt> = Vec::new();
        let regs = [0u32; 16];
        push_choice(&mut stack, &regs, 10, 0, 0, 0, 0);
        update_alt(&mut stack, 99);
        assert_eq!(top_alt(&stack), Some(99));
    }

    #[test]
    fn binding_beyond_saved_hp_is_untrailed_on_restore() {
        let mut heap: Vec<u32> = Vec::new();
        let mut trail: Vec<u32> = Vec::new();
        let r = alloc_unbound(&mut heap);
        let mut stack: Vec<ChoicePt> = Vec::new();
        let regs = [0u32; 16];
        push_choice(&mut stack, &regs, 0, 0, heap.len(), trail.len(), 0);
        bind(0, make(TAG_ATOM, 1), &mut heap, &mut trail);
        assert!(!is_unbound(r, &heap));
        let mut regs = [0u32; 16];
        restore_top(&stack, &mut regs, &mut heap, &mut trail, noop_env);
        assert_eq!(deref(r, &heap), r);
    }

    #[test]
    fn restore_truncates_env_stack() {
        let mut stack: Vec<ChoicePt> = Vec::new();
        let regs = [0u32; 16];
        let heap: Vec<u32> = Vec::new();
        let trail: Vec<u32> = Vec::new();
        push_choice(&mut stack, &regs, 0, 0, heap.len(), trail.len(), 3);
        let mut heap = Vec::new();
        let mut trail = Vec::new();
        let mut saw: Option<usize> = None;
        let mut regs = [0u32; 16];
        restore_top(&stack, &mut regs, &mut heap, &mut trail, |ep| saw = Some(ep));
        assert_eq!(saw, Some(3));
    }
}
