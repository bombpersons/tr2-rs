#!/usr/bin/env python3
"""Check natural fast falls and slide-edge landings at retained PSX poses."""
from backward_collision_smoke import main

CAPTURES = {
    'pit_jump': [24, 70, 106, 108, 110, 118, 120, 140, 190],
    'ramp_run': [30, 52, 70, 72, 76, 78, 90, 120, 160, 230],
    'ramp_hop_back': [24, 46, 62, 64, 72, 80, 116, 122, 164, 230],
    'pit_angled_heldout': [40, 110, 124, 126, 140, 160, 210],
    'ramp_run_late_heldout': [70, 100, 110, 112, 118, 130, 160, 200, 240],
    'ramp_back_near_heldout': [30, 60, 80, 100, 120, 140, 180, 220],
}

if __name__ == '__main__':
    main(CAPTURES, 'fall', min_colors=8)
