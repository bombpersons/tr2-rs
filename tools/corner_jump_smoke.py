#!/usr/bin/env python3
"""Check airborne side/corner contacts at retained PSX poses in the full game."""
from backward_collision_smoke import main

CAPTURES = {
    'low_left': [26, 28, 34, 36, 40, 44, 46, 50, 90, 160],
    'open_left': [28, 30, 32, 36, 40, 52, 70, 84, 86, 160],
    'open_reach': [28, 32, 36, 40, 42, 52, 70, 84, 86, 180],
    'open_right_heldout': [28, 30, 32, 36, 40, 50, 70, 90, 160],
    'low_right_delay_heldout': [44, 46, 50, 54, 58, 62, 70, 100, 180],
    'open_reach_release_heldout': [36, 40, 46, 52, 92, 136, 160, 172, 174, 176, 220],
    'open_right_reach_validation_heldout': [32, 36, 40, 48, 60, 90, 140, 180, 200, 240],
}

if __name__ == '__main__':
    main(CAPTURES, 'corner', min_colors=8)
