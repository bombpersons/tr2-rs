#!/usr/bin/env python3
"""Check side jumps, directional standing jumps, running twists and hop-fall reaches at retained PSX poses."""
from backward_collision_smoke import main

CAPTURES = {
    'left_pit': [10, 20, 40, 108, 120, 190],
    'right_block': [10, 20, 60, 104, 110],
    'right_wall': [10, 30, 70, 110],
    'run_twist': [46, 62, 80, 96, 104, 130],
    'hop_reach': [20, 34, 52, 100, 190],
    'right_pit_heldout': [20, 60, 108, 150],
    'left_wall_heldout': [24, 60, 110],
    'left_over_right_heldout': [18, 40, 110],
    'forward_over_right_heldout': [18, 40, 76, 120],
    'wall_then_back_heldout': [18, 40, 76, 120],
    'standing_back_heldout': [18, 50, 108, 150],
    'side_twist_heldout': [26, 60, 120],
    'run_twist_roll_heldout': [50, 70, 96, 110],
    'run_turn_jump_heldout': [46, 70, 102, 140],
    'hop_reach_late_heldout': [42, 60, 66, 100],
    'walk_jump_heldout': [10, 30, 56, 100],
}

if __name__ == '__main__':
    main(CAPTURES, 'side_jump', min_colors=8)
