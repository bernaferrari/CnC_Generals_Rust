#include "wlib/xtime.h"
#include "timezone.h"

void GetTimezoneInfo(const char * &timezone_str, int &timezone_offset) {
	timezone_str = "Unknown Timezone";
	timezone_offset = 0;
#ifdef _WINDOWS
	struct _timeb    wintime;
	_ftime(&wintime);

	if (wintime.dstflag) {
		// Daylight savings time
		if (_daylight) {
			timezone_str = _tzname[1];
		}
	} else {
		timezone_str = _tzname[0];
	}
	timezone_offset = wintime.timezone * 60; // its in minutes...

#endif
#ifndef _WINDOWS
	struct timeval   unixtime;
	struct timezone  unixtzone;
	gettimeofday(&unixtime,&unixtzone);

	struct tm unixtm;
	localtime_r(&unixtime.tv_sec, &unixtm);

	if (unixtm.tm_isdst) {
		// Daylight savings time
		if (daylight) timezone_str = tzname[1];
		timezone_offset = altzone;
	} else {
		timezone_str = tzname[0];
		timezone_offset = timezone;
	}
#endif
}

const char * TimezoneString(void) {
	const char *timezone_str;
	int timezone_offset;
	GetTimezoneInfo(timezone_str, timezone_offset);
	return timezone_str;
}

int TimezoneOffset(void) {
	const char *timezone_str;
	int timezone_offset;
	GetTimezoneInfo(timezone_str, timezone_offset);
	return timezone_offset;
}
