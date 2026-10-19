#!/usr/bin/env python3
"""Check airborne collision poses in the full game using retained PSX cameras."""
from backward_collision_smoke import main

CAPTURES = {
    'vertical_steer_forward': [28, 30, 32, 34, 40, 42, 52, 90, 100, 102, 144, 220],
    'vertical_steer_back_open': [28, 30, 32, 34, 36, 40, 42, 52, 90, 100, 102, 144, 220],
    'vertical_steer_wall': [30, 36, 38, 40, 42, 60, 82, 84, 86, 90, 130, 220],
    'vertical_steer_reverse_heldout': [46, 48, 50, 52, 54, 56, 58, 60, 76, 116, 118, 120, 162, 240],
    'vertical_steer_late_heldout': [44, 46, 54, 56, 60, 62, 64, 80, 108, 116, 118, 160, 240],
    'braking_wall_validation_heldout': [190, 216, 218, 224, 226, 228, 230, 246, 256],
    'vertical_clearance_heldout': [24, 28, 30, 36, 38, 40, 62, 82, 84, 86, 98, 100, 102, 142, 144, 210],
    'vertical_clearance_delay_heldout': [42, 46, 48, 54, 56, 58, 80, 100, 102, 104, 116, 118, 120, 160, 162, 250],
    'forward_ledge_release_heldout': [80, 116, 140, 184, 194, 196, 198, 220],
    'forward_ledge_walk_heldout': [58, 60, 78, 80, 82, 116, 118, 130, 132, 170, 172, 174, 204, 206, 208],
    'walk_stop_left_heldout': [100, 102, 126, 128, 130, 158, 160, 162, 220],
    'walk_stop_right_heldout': [80, 82, 90, 92, 122, 124, 126, 200],
    'walk_stop_rehold_heldout': [80, 92, 110, 112, 122, 124, 126, 162, 164, 178, 180, 182, 212, 214, 216, 260],
    'forward_ledge_walk_validation_heldout': [62, 64, 72, 74, 94, 96, 98, 132, 134, 148, 150, 188, 190, 220, 222, 224, 260],
    'braking_wall_release_validation_heldout': [196, 198, 200, 206, 208, 210, 212, 214, 260],
    'forward_ledge_run': [46, 48, 50, 58, 78, 80, 82, 92, 100, 102, 132, 134, 136, 190],
    'forward_ledge_delay_heldout': [60, 62, 64, 72, 92, 94, 96, 112, 114, 142, 172, 210],
    'forward_ledge_release_validation_heldout': [60, 62, 68, 76, 84, 92, 94, 96, 128, 130, 132, 148, 160, 170, 210],
    'run_stop_left_heldout': [58, 60, 62, 70, 90, 92, 94, 96, 180],
    'run_stop_right_heldout': [78, 80, 82, 90, 110, 112, 114, 116, 200],
    'run_stop_rehold_heldout': [56, 58, 70, 72, 88, 90, 92, 114, 116, 120, 128, 160, 220],
    'run_stop_rehold_validation_heldout': [64, 66, 76, 78, 90, 92, 94, 136, 138, 150, 180, 240],
    'run_stop_restart_heldout': [66, 68, 88, 90, 92, 94, 138, 140, 160, 180, 240],
    'run_start_retry_probe': [24, 26, 28, 30, 50, 58, 94, 130, 170],
    'run_start_release_heldout': [24, 28, 30, 50, 78, 82, 150],
    'run_start_hold_heldout': [26, 28, 30, 52, 58, 96, 100, 138, 174, 240],
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
