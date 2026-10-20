#!/usr/bin/env python3
"""Check slide jumps, mid-air twists and back-flight fast falls at retained PSX poses."""
from backward_collision_smoke import main

CAPTURES = {
    'forward': [72, 74, 78, 90, 110, 140, 170],
    'back': [64, 66, 72, 90, 108, 112, 150],
    'twist_pit': [26, 36, 46, 70, 108, 120, 190],
    'twist_slide_back': [66, 74, 86, 96, 110, 140, 160],
    'forward_away_heldout': [72, 80, 90, 120, 160],
    'forward_late_heldout': [76, 80, 100, 140, 170],
    'back_held_heldout': [72, 78, 100, 116, 150],
    'back_away_heldout': [64, 70, 82, 122, 162],
    'twist_roll_heldout': [28, 46, 70, 108, 120],
    'twist_late_heldout': [36, 60, 108, 120, 190],
    'back_twist_roll_heldout': [80, 92, 102, 116, 140, 166],
    'hop_pit_heldout': [10, 22, 50, 66, 72, 150],
}

if __name__ == '__main__':
    main(CAPTURES, 'slide_jump', min_colors=8)
