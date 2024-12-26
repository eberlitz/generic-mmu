# Generic MMU for 3D printers

The whole controlling logic can be found at [src/filament_changer.rs](../src/filament_changer.rs).
ESP32 pinout configuration can be found at [src/main.rs](../src/main.rs).

Bill of materials:

| qty | product                                           | link                                                  |
| --- | ------------------------------------------------- | ----------------------------------------------------- |
| 2   | Nema17 17HS4023                                   | https://www.aliexpress.com/item/1005005907696088.html |
| 1   | Bambu Lab 3D printer Part 4-in-1 PTFE Adapter     | https://www.aliexpress.com/item/1005006962712897.html |
| 4   | M4 12mm Hex Bolt                                  |                                                       |
| 1   | MK8 Extruder Drive Gear 9mmx11mm 5mm hole         | https://www.aliexpress.com/item/33001874597.html      |
| 1   | DC 12V to 3.3V 5V 12V Step Down Buck Power Supply |                                                       |
| 1   | DC 12V 3A Power Supply                            | https://www.aliexpress.com/item/1005007419359413.html |
| 2   | BIGTREETECH TMC2208 V3.0                          |                                                       |
| 1   | ESP32 WROOM 32 CH340                              | https://www.amazon.de/dp/B0D9BTQRYT                   |
| 1   | LM2596S DC-DC Power Supply Step Down              | https://www.amazon.de/dp/B089QJM8KQ                   |

Pseudo schematics:

```
POWER SUPPLY (12V 3A)
  |
  +---> (+) Input of 12V-5V-3.3V Buck Converter
  |
  +---> (+) Input of 12V-6V Buck Converter
  |
  GND--> (-) Input of 12V-5V-3.3V Buck Converter
  |
  GND--> (-) Input of 12V-6V Buck Converter

12V-5V-3.3V BUCK CONVERTER
  |
  +-- 12V Output --> TMC2208 (VMOT), Capacitor (Positive Lead)
  |
  +-- 5V Output  --> ESP32 (VIN)
  |
  +-- 3.3V Output --> TMC2208 (VIO), TMC2208 (MS1), TMC2208 (MS2)
  |
  GND-------------> Common Ground (Connect to all GND points below)

12V-6V BUCK CONVERTER
  |
  +-- 6V Output  --> Servo (Red Wire)
  |
  GND-------------> Common Ground (Connect to all GND points below)

CAPACITOR (e.g., 100uF, 16V)
  |
  +-- Positive Lead --> TMC2208 (VMOT)
  |
  +-- Negative Lead --> Common Ground

ESP32
  |
  +-- VIN  <---------- 5V (from 12V-5V-3.3V Buck Converter)
  |
  +-- GND  <---------- Common Ground
  |
  +-- GPIO2  ---------> (Internal Onboard LED)
  |
  +-- GPIO15 ---------> TMC2208 (Stepper A - DIR)
  |
  +-- GPIO4  ---------> TMC2208 (Stepper A - STEP)
  |
  +-- GPIO16 ---------> TMC2208 (Stepper A - EN)
  |
  +-- GPIO17 ---------> TMC2208 (Stepper B - DIR)
  |
  +-- GPIO5  ---------> TMC2208 (Stepper B - STEP)
  |
  +-- GPIO18 ---------> TMC2208 (Stepper B - EN)
  |
  +-- GPIO23 ---------> Servo (Signal - Orange Wire)
  |
  +-- GPIO19 ---------> Endstop Switch (Signal)

TMC2208 (Stepper A)
  |
  +-- VMOT <---------- 12V (from 12V-5V-3.3V Buck Converter), Capacitor (Positive Lead)
  |
  +-- GND  <---------- Common Ground, Capacitor (Negative Lead)
  |
  +-- VIO  <---------- 3.3V (from 12V-5V-3.3V Buck Converter)
  |
  +-- DIR  <---------- GPIO15 (from ESP32)
  |
  +-- STEP <---------- GPIO4 (from ESP32)
  |
  +-- EN   <---------- GPIO16 (from ESP32)
  |
  +-- MS1  <---------- 3.3V (from 12V-5V-3.3V Buck Converter)
  |
  +-- MS2  <---------- 3.3V (from 12V-5V-3.3V Buck Converter)
  |
  +-- MS3  <---------- Common Ground
  |
  +-- A1   -----------> Stepper Motor A (Coil A - Pin 1)
  |
  +-- A2   -----------> Stepper Motor A (Coil A - Pin 2)
  |
  +-- B1   -----------> Stepper Motor A (Coil B - Pin 1)
  |
  +-- B2   -----------> Stepper Motor A (Coil B - Pin 2)

TMC2208 (Stepper B)
  |
  +-- VMOT <---------- 12V (from 12V-5V-3.3V Buck Converter), Capacitor (Positive Lead)
  |
  +-- GND  <---------- Common Ground, Capacitor (Negative Lead)
  |
  +-- VIO  <---------- 3.3V (from 12V-5V-3.3V Buck Converter)
  |
  +-- DIR  <---------- GPIO17 (from ESP32)
  |
  +-- STEP <---------- GPIO5 (from ESP32)
  |
  +-- EN   <---------- GPIO18 (from ESP32)
  |
  +-- MS1  <---------- 3.3V (from 12V-5V-3.3V Buck Converter)
  |
  +-- MS2  <---------- 3.3V (from 12V-5V-3.3V Buck Converter)
  |
  +-- MS3  <---------- Common Ground
  |
  +-- A1   -----------> Stepper Motor B (Coil A - Pin 1)
  |
  +-- A2   -----------> Stepper Motor B (Coil A - Pin 2)
  |
  +-- B1   -----------> Stepper Motor B (Coil B - Pin 1)
  |
  +-- B2   -----------> Stepper Motor B (Coil B - Pin 2)

SERVO MG996R
  |
  +-- Red Wire    <----- 6V (from 12V-6V Buck Converter)
  |
  +-- Brown Wire  <----- Common Ground
  |
  +-- Orange Wire <----- GPIO23 (from ESP32)

ENDSTOP SWITCH
  |
  +-- Signal <---------- GPIO19 (from ESP32)
  |
  +-- GND    <---------- Common Ground
```

Note: Stepper A is the filament selector and Stepper B is the one that loads/unloads the filament.
