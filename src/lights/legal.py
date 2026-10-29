# lights - brightness controller for monitors via ddcutil
# Copyright (C) 2026  Armand Burger
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Copyright and licence notices (GNU GPL v3 "Appropriate Legal Notices")."""
from __future__ import annotations

from importlib import metadata

COPYRIGHT = "lights  Copyright (C) 2026  Armand Burger"
CONTACT = "GitHub @ArmandBurger, Twitter/X @ghastly_dev"
LICENSE_URL = "https://www.gnu.org/licenses/gpl-3.0.txt"


def notice(show_w: str, show_c: str) -> str:
    """The short notice the GPL asks interactive programs to show on start-up.

    ``show_w``/``show_c`` say how to view the warranty and the licence
    conditions in the current interface, e.g. "type `lights show w'".
    """
    return (
        f"{COPYRIGHT}\n"
        f"This program comes with ABSOLUTELY NO WARRANTY; for details {show_w}.\n"
        "This is free software, and you are welcome to redistribute it\n"
        f"under certain conditions; {show_c} for details."
    )

VERSION_TEXT = f"""\
{COPYRIGHT}
License GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>.
This is free software: you are free to change and redistribute it.
There is NO WARRANTY, to the extent permitted by law.
Contact: {CONTACT}"""

# Sections 15 and 16 of the GNU GPL v3, verbatim.
WARRANTY = """\
  15. Disclaimer of Warranty.

  THERE IS NO WARRANTY FOR THE PROGRAM, TO THE EXTENT PERMITTED BY
APPLICABLE LAW.  EXCEPT WHEN OTHERWISE STATED IN WRITING THE COPYRIGHT
HOLDERS AND/OR OTHER PARTIES PROVIDE THE PROGRAM "AS IS" WITHOUT WARRANTY
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
"""


def license_text() -> str:
    """The full GPL v3 text, as installed with the package's metadata."""
    try:
        text = metadata.distribution("lights").read_text("licenses/LICENSE")
    except metadata.PackageNotFoundError:
        text = None
    if text:
        return text
    return f"The GNU General Public License v3 is available at <{LICENSE_URL}>.\n"
