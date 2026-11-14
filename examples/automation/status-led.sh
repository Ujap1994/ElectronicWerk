#!/bin/sh
# A status LED from the first symbol to the Gerber files, with the ewerk
# command-line tool. Usage: sh status-led.sh [project folder]
set -e
P="${1:-Status LED}"

ewerk project new "$P" --width 30 --height 20

# Schematic: parts from the project library, footprints from the wizard.
ewerk schematic place "$P" R --value 330 --footprint 0805
ewerk schematic place "$P" LED --value green --footprint 0805
ewerk schematic place "$P" C --value 100n --footprint 0603
ewerk schematic connect "$P" VCC R1.1 C1.1 --style power
ewerk schematic connect "$P" LED_A R1.2 D1.A
ewerk schematic connect "$P" GND D1.K C1.2 --style power
ewerk schematic erc "$P"

# Board: footprints onto the board (positions in mm), routed, checked.
ewerk board update "$P"
ewerk board place "$P" R1 --at 10,8
ewerk board place "$P" D1 --at 20,8 --rotate 180
ewerk board place "$P" C1 --at 15,14
ewerk board route "$P"
ewerk board drc "$P"

# Manufacturing files.
ewerk outputs run "$P"
