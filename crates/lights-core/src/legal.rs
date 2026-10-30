// lights - brightness controller for monitors via ddcutil
// Copyright (C) 2026  Armand Burger
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Copyright, licence and third-party notice texts. The full licence and the
//! notices are compiled into the binary, so they are always available.

pub const COPYRIGHT: &str = "lights Copyright (C) 2026 Armand Burger";
pub const CONTACT: &str = "GitHub @ArmandBurger, Twitter/X @ghastly_dev";
pub const WEBSITE: &str = "https://github.com/ArmandBurger";

/// The full GNU GPL v3 text.
pub const LICENSE: &str = include_str!("../../../LICENSE");
/// Copyright and licence of bundled third-party material.
pub const NOTICES: &str = include_str!("../../../THIRD_PARTY_NOTICES.md");

/// The short notice the GPL asks interactive programs to show on start-up.
///
/// `show_w`/`show_c` say how to view the warranty and the licence
/// conditions in the current interface, e.g. "type `lights show w'".
pub fn notice(show_w: &str, show_c: &str) -> String {
    format!(
        "{COPYRIGHT}\n\
         This program comes with ABSOLUTELY NO WARRANTY; for details {show_w}.\n\
         This is free software, and you are welcome to redistribute it\n\
         under certain conditions; {show_c} for details."
    )
}

/// The licence notice, third-party notices and full licence, as shown under About.
pub fn about() -> String {
    [notice_about().as_str(), NOTICES, LICENSE].join("\n\n")
}

fn notice_about() -> String {
    format!(
        "{COPYRIGHT}

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License (below) for more details."
    )
}

pub fn version_text() -> String {
    format!(
        "{COPYRIGHT}
License GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>.
This is free software: you are free to change and redistribute it.
There is NO WARRANTY, to the extent permitted by law.
Contact: {CONTACT}"
    )
}

/// Sections 15 and 16 of the GNU GPL v3, verbatim.
pub const WARRANTY: &str = "  15. Disclaimer of Warranty.

  THERE IS NO WARRANTY FOR THE PROGRAM, TO THE EXTENT PERMITTED BY
APPLICABLE LAW.  EXCEPT WHEN OTHERWISE STATED IN WRITING THE COPYRIGHT
HOLDERS AND/OR OTHER PARTIES PROVIDE THE PROGRAM \"AS IS\" WITHOUT WARRANTY
OF ANY KIND, EITHER EXPRESSED OR IMPLIED, INCLUDING, BUT NOT LIMITED TO,
THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
PURPOSE.  THE ENTIRE RISK AS TO THE QUALITY AND PERFORMANCE OF THE PROGRAM
IS WITH YOU.  SHOULD THE PROGRAM PROVE DEFECTIVE, YOU ASSUME THE COST OF
ALL NECESSARY SERVICING, REPAIR OR CORRECTION.

  16. Limitation of Liability.

  IN NO EVENT UNLESS REQUIRED BY APPLICABLE LAW OR AGREED TO IN WRITING
WILL ANY COPYRIGHT HOLDER, OR ANY OTHER PARTY WHO MODIFIES AND/OR CONVEYS
THE PROGRAM AS PERMITTED ABOVE, BE LIABLE TO YOU FOR DAMAGES, INCLUDING ANY
GENERAL, SPECIAL, INCIDENTAL OR CONSEQUENTIAL DAMAGES ARISING OUT OF THE
USE OR INABILITY TO USE THE PROGRAM (INCLUDING BUT NOT LIMITED TO LOSS OF
DATA OR DATA BEING RENDERED INACCURATE OR LOSSES SUSTAINED BY YOU OR THIRD
PARTIES OR A FAILURE OF THE PROGRAM TO OPERATE WITH ANY OTHER PROGRAMS),
EVEN IF SUCH HOLDER OR OTHER PARTY HAS BEEN ADVISED OF THE POSSIBILITY OF
SUCH DAMAGES.
";
