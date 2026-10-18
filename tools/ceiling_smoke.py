#!/usr/bin/env python3
"""Check forward low-ceiling collision in the standalone game, at PSX poses."""
from backward_collision_smoke import main

CAPTURES = {
    'forward': [24, 26, 28, 30, 40, 46, 50, 70, 160],
    'forward_delay_heldout': [40, 42, 44, 46, 56, 62, 66, 100, 180],
    'forward_release_heldout': [24, 28, 30, 46, 50, 56, 60, 66, 100, 180],
}

if __name__ == '__main__':
    main(CAPTURES, 'ceiling', min_colors=8)
