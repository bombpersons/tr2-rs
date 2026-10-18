#!/usr/bin/env python3
"""Check airborne collision poses in the full game using retained PSX cameras."""
from backward_collision_smoke import main

CAPTURES = {
    'middle_wall': [24, 30, 32, 36, 42, 50, 86],
    'running_low_wall': [46, 58, 60, 62, 64, 68, 100, 102],
    'blocked_takeoff': [14, 24, 30, 84, 102, 142],
    'middle_delay_heldout': [40, 46, 60, 74, 100],
    'low_cancel_heldout': [60, 62, 70, 84, 100, 104, 150],
    'low_cancel_validation_heldout': [60, 64, 70, 90, 102, 166, 180, 240],
    'forward_wall': [24, 56, 58, 60, 62, 78, 88, 110, 168],
    'reach_wall': [40, 58, 64, 86, 88, 90, 126],
    'forward_steer': [34, 48, 58, 60, 64, 88, 100, 180],
    'forward_delay_heldout': [72, 74, 102, 110, 150, 182, 200],
    'reach_release_heldout': [64, 66, 88, 90, 126, 140, 180, 220],
}

if __name__ == '__main__':
    main(CAPTURES, 'airborne')
