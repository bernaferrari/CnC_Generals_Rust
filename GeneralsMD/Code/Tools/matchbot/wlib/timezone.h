/****************************************************************************\
timezone                      Matthew D. Campbell

This is just a couple of convenience functions for determining what timezone
we are now in.  It even accounts for daylight savings!  One caveat is that it
only tells you info about what the daylight savings info is now, not 5 minutes
from now, not 2 hours ago.  Oh well.
\****************************************************************************/

#ifndef _TIMEZONE_H_
#define _TIMEZONE_H_

// Just fill in both the timezone description and its offset from GMT
void GetTimezoneInfo(const char * &timezone_str, int &timezone_offset);

// Returns the description of the current timezone (daylight savings included)
const char * TimezoneString(void);

// Returns the offset from GMT of the current timezone
int TimezoneOffset(void);

#endif // _TIMEZONE_H_

