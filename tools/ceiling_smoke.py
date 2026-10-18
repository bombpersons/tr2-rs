#!/usr/bin/env python3
"""Check forward, vertical and reach ceiling collision at retained PSX poses."""
from backward_collision_smoke import main

CAPTURES = {
    'reach': [28, 40, 44, 46, 48, 82, 84, 124, 180],
    'reach_late': [40, 50, 52, 54, 58, 82, 84, 86, 122, 124, 200],
    'reach_late_release_heldout': [44, 52, 58, 84, 86, 128, 130, 160, 162, 166, 180, 198, 220],
    'reach_late_delay_heldout': [56, 66, 68, 70, 100, 102, 108, 118, 138, 140, 220],
    'reach_late_validation_heldout': [50, 60, 66, 92, 94, 136, 138, 168, 170, 174, 190, 206, 220],
    'vertical': [28, 30, 32, 34, 36, 40, 72, 74, 160],
    'vertical_delay_heldout': [28, 44, 46, 48, 50, 52, 60, 88, 90, 180],
    'vertical_hold_heldout': [30, 32, 34, 36, 40, 56, 58, 60, 82, 84, 86, 110, 118, 136, 138, 180],
    'forward': [24, 26, 28, 30, 40, 46, 50, 70, 160],
    'forward_delay_heldout': [40, 42, 44, 46, 56, 62, 66, 100, 180],
    'forward_release_heldout': [24, 28, 30, 46, 50, 56, 60, 66, 100, 180],
}

if __name__ == '__main__':
    main(CAPTURES, 'ceiling', min_colors=8)
