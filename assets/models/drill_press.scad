// Drill press - all working parts
// Units: mm
// Coordinate system: origin at base centre, Z up.

$fn = 48;

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------
base_w        = 300;   // base plate width  (X)
base_d        = 240;   // base plate depth  (Y)
base_h        = 22;    // base plate thickness
column_d      = 60;    // column diameter
column_h      = 950;   // column height above base top
table_w       = 220;   // work table width
table_d       = 180;   // work table depth
table_h       = 26;    // work table thickness
collar_h      = 60;    // table locking collar height
head_h        = 130;   // head casting height (front-to-back block)
head_d        = 170;   // head casting depth
head_w        = 190;   // head casting width
spindle_d     = 26;    // spindle diameter
spindle_len   = 150;   // spindle travel + overhang
chuck_d       = 46;    // chuck diameter
chuck_h       = 62;    // chuck height
bit_d         = 8;     // drill bit diameter
bit_len       = 90;    // drill bit length
hub_d         = 22;    // hub/column diameter for the head
hub_h         = 40;    // hub boss height under the head
pulley_r      = 45;    // motor pulley radius (stepped; outer shown)
quill_r       = 34;    // quill collar radius

// ---- derived geometry: single source of truth for every part ---------------
column_top   = base_h + column_h;             // top of the column
head_bottom  = column_top + hub_h;            // underside of the head body
spindle_x    = 0;                             // spindle centre X
spindle_y    = head_d / 2 - 30;               // spindle centre Y (front overhang)
table_top    = base_h + 380;                  // work table surface height
quill_top    = head_bottom + head_h * 0.8;    // top of the quill, rides inside the head
pulley_level = head_bottom + head_h * 0.75;   // pulley/belt height (inside the head)
spindle_pulley_y = spindle_y - 22;            // spindle pulley centre Y

// ---------------------------------------------------------------------------
// Modules
// ---------------------------------------------------------------------------

// Base plate with a T-slot style centre hole and four bolt pads.
module base_plate() {
    difference() {
        union() {
            // main slab with rounded corners
            linear_extrude(height = base_h)
                offset(r = 18) offset(delta = -18)
                    square([base_w, base_d], center = true);
            // raised bolt pads in the corners
            for (sx = [-1, 1], sy = [-1, 1])
                translate([sx * (base_w / 2 - 26), sy * (base_d / 2 - 26), base_h])
                    cylinder(d = 34, h = 6);
        }
        // centre clearance hole under the column
        translate([0, 0, -1]) cylinder(d = column_d + 6, h = base_h + 12);
        // mounting bolt holes
        for (sx = [-1, 1], sy = [-1, 1])
            translate([sx * (base_w / 2 - 26), sy * (base_d / 2 - 26), -1])
                cylinder(d = 11, h = base_h + 20);
        // front edge recess (chip clear-out)
        translate([0, base_d / 2 - 12, -1])
            cube([base_w * 0.5, 26, base_h + 2], center = true);
    }
}

// Column: ground steel round bar.
module column() {
    translate([0, 0, base_h])
        cylinder(d = column_d, h = column_h);
}

// Column rack (the toothed strip the table/head feed rides on).
module rack() {
    translate([0, -(column_d / 2 + 5), base_h + 120])
        cube([18, 10, column_h - 200], center = false);
    // rack teeth (a simplified comb, pointing toward the column)
    for (i = [0 : 11 : column_h - 220])
        translate([0, -(column_d / 2 + 0.5), base_h + 132 + i])
            cube([14, 8, 4], center = false);
}

// Work table: cast table with T-slots, mounted on a collar around the column.
module work_table() {
    z = table_top;
    union() {
        // locking collar clamp
        translate([0, 0, z - table_h - collar_h + 14])
            cylinder(d = column_d + 26, h = collar_h + table_h);
        // clamp bolt
        translate([column_d / 2 + 14, 0, z - table_h - collar_h + 20])
            rotate([0, 90, 0]) cylinder(d = 12, h = 46);
        // table plate
        translate([0, 0, z - table_h])
            linear_extrude(height = table_h)
                offset(r = 10) offset(delta = -10)
                    translate([0, 0]) square([table_w, table_d], center = true);
        // table stiffening rib underneath
        translate([0, 0, z - table_h - 10])
            cube([table_w * 0.5, table_d * 0.5, 12], center = true);
    }
}

// T-slot cutters for the table (subtracted from the plate).
module table_slots(z) {
    for (ox = [-70, 0, 70]) {
        translate([ox, 0, z - table_h - 1])
            cube([12, table_d - 24, table_h + 2], center = false);
        translate([ox, 0, z - 16])
            cube([20, table_d - 24, 8], center = true);
    }
}

// Head casting: holds the spindle assembly, sits on top of the column.
module head_casting() {
    z = column_top;
    bottom = head_bottom - head_h / 2;
    union() {
        // hub boss clamping the column
        translate([0, 0, z]) cylinder(d = column_d + 34, h = hub_h + 8);
        // main body, overhanging forward (toward +Y) for the spindle
        translate([0, head_d / 2 - 30, bottom]) cube([head_w, head_d, head_h], center = true);
        // motor mount pad on top, behind the spindle
        translate([0, -30, bottom + head_h]) cube([120, 90, 14], center = true);
        // quill housing boss on the front of the body
        translate([spindle_x, spindle_y, bottom])
            cylinder(d = quill_r * 2 + 18, h = head_h + 14);
    }
}

module step_pulley(r, h, steps) {
    step = h / steps;
    for (i = [0 : steps - 1])
        translate([0, 0, i * step])
            cylinder(r = r - i * (r * 0.18), h = step);
}

// Motor body + pulley.
module motor() {
    z = head_bottom - head_h / 2 + head_h + 14;   // top of the motor pad
    translate([0, -30, z]) {
        // motor can, lying horizontally
        translate([0, 0, 60]) rotate([0, 90, 0])
            cylinder(d = 90, h = 130, center = true);
        for (i = [0 : 9])
            translate([-65 + i * 14, 0, 60]) rotate([0, 90, 0])
                cylinder(d = 104, h = 4, center = true);
        // Drive pulley on the motor shaft, centred on pulley_level. The 90 deg
        // Y rotation maps the pulley's build axis (+Z) onto +X, so its 42 mm
        // height lies along X and the half-height (21) shift goes on X too.
        translate([-20 - 21, -12, pulley_level - z]) rotate([0, 90, 0])
            step_pulley(pulley_r, 42, 3);
    }
}

// V-belt running between the motor pulley and the spindle pulley, in a plane
// that passes through the column bore so it never intersects the head casting.
module belt_top() {
    // centred on the motor pulley band (pulley_level + 20 above the pad)
    translate([0, 0, pulley_level + 20])
        rotate([90, 0, 0])
            linear_extrude(height = 8, center = true)
                translate([0, 0])
                    difference() {
                        hull() {
                            translate([spindle_x, 0])   circle(r = pulley_r + 2);
                            translate([spindle_x - 88, 0]) circle(r = pulley_r + 2);
                            translate([0, -40])          circle(r = pulley_r + 2);
                        }
                        hull() {
                            translate([spindle_x, 0])   circle(r = pulley_r - 6);
                            translate([spindle_x - 88, 0]) circle(r = pulley_r - 6);
                            translate([0, -40])          circle(r = pulley_r - 6);
                        }
                    }
}

// Spindle + chuck + bit: the quill assembly hanging under the head body.
// The quill collar stays inside the head; the chuck and bit hang below it so
// the bit tip sits just above the work table.
module spindle_assembly(feed = 0) {
    z = head_bottom;
    translate([spindle_x, spindle_y, z + feed]) {
        // quill collar (rides up inside the head bore)
        cylinder(d = quill_r * 2, h = quill_top - head_bottom);
        // spindle shaft leaving the head
        translate([0, 0, -spindle_len + 40])
            cylinder(d = spindle_d, h = spindle_len - 20);
        // chuck body
        translate([0, 0, -spindle_len - chuck_h + 40]) {
            cylinder(d1 = chuck_d, d2 = chuck_d * 1.02, h = chuck_h);
            for (i = [0 : 2]) rotate([0, 0, i * 120])
                translate([chuck_d * 0.18, 0, -8]) cylinder(d = 9, h = 18);
        }
        // drill bit
        translate([0, 0, -spindle_len - chuck_h - bit_len + 40]) {
            translate([0, 0, bit_len * 0.45]) cylinder(d = bit_d, h = bit_len * 0.55);
            cylinder(d1 = bit_d * 0.55, d2 = bit_d, h = bit_len * 0.5);
            for (i = [0 : 3]) rotate([0, 0, i * 90])
                translate([0, 0, bit_len * 0.08])
                    linear_extrude(height = bit_len * 0.5, twist = 220, slices = 24)
                        translate([bit_d * 0.32, 0, 0])
                            square([bit_d * 0.22, bit_d * 0.22], center = true);
        }
    }
}

// Upper spindle: bearing sleeve and the driven step pulley above the head.
module spindle_upper() {
    translate([spindle_x, spindle_pulley_y, pulley_level - 6]) {
        // bearing sleeve rising out of the head
        cylinder(d = 58, h = pulley_level - head_bottom + 40);
        // driven step pulley on the spindle
        translate([0, 0, 34]) step_pulley(pulley_r, 42, 3);
        // spindle nose above the pulley
        translate([0, 0, 76]) cylinder(d = spindle_d, h = 18);
    }
}

// Feed handle assembly: three-spoke crank whose pinion enters the head's
// right-hand side and drives the quill.
module feed_handle() {
    z = head_bottom + head_h * 0.55;
    translate([head_w / 2, spindle_y, z]) {
        // pinion boss entering the head
        rotate([0, 90, 0]) cylinder(d = 34, h = 26);
        // hub outside the head
        translate([26, 0, 0]) rotate([0, 90, 0]) cylinder(d = 44, h = 24);
        // three spokes in the XZ plane, outboard of the head
        for (a = [0, 120, 240])
            rotate([a, 0, 0])
                translate([50, 0, 0]) {
                    rotate([0, 90, 0]) cylinder(d = 12, h = 78, center = true);
                    translate([39, 0, 0]) sphere(d = 26);   // grip knob
                }
    }
}

// Depth-stop rod and lock nut on the head's left side.
module depth_stop() {
    z = head_bottom - head_h / 2 + 20;
    translate([-(head_w / 2 - 10), spindle_y, z]) {
        cylinder(d = 10, h = 240);
        translate([0, 0, 160]) rotate([0, 0, 0]) cylinder(d = 26, h = 22);
        translate([0, 0, 120]) cylinder(d = 22, h = 10);
    }
}

// Chuck key lying beside the base (a working part, shown where it stows).
module chuck_key() {
    translate([base_w / 2 - 60, -base_d / 2 + 40, base_h + 4]) {
        rotate([0, 0, 20]) {
            cylinder(d = 9, h = 90, center = false);
            translate([0, 0, 84]) rotate([0, 90, 0]) cylinder(d = 9, h = 40);
            translate([0, 0, 84]) rotate([0, 90, 0]) translate([36, 0, 0])
                cylinder(d = 12, h = 26);
        }
    }
}

// ---------------------------------------------------------------------------
// Assembly
// ---------------------------------------------------------------------------
module drill_press() {
    difference() {
        union() {
            base_plate();
            column();
            rack();
            work_table();
            head_casting();
            motor();
            belt_top();
            spindle_assembly(0);
            spindle_upper();
            feed_handle();
            depth_stop();
            chuck_key();
        }
        table_slots(table_top);
        // bore the collar/head for the column so the parts read as fitted
        translate([0, 0, table_top - table_h - collar_h + 12])
            cylinder(d = column_d + 1, h = collar_h + table_h + 4);
        translate([0, 0, column_top - 1])
            cylinder(d = column_d + 1, h = hub_h + head_h + 20);
    }
}

drill_press();
