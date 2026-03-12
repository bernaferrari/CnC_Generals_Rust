#include "strtok_r.h"
#include <string.h>
#include <stdio.h>

//
// Replacement for strtok() that doesn't use a static to
//   store the current position.  The name comes from the
//   POSIX threadsafe version of strtok (r = reentrant).  The
//   user provided var lasts is used in place of the static.
//
// Yes the Windows version of strtok is already threadsafe,
//   but the fact that you can't call a function that uses strtok()
//   during a series of strtok() calls is really annoying.
//
#ifndef _UNIX
char *strtok_r(char *strptr, const char *delimiters, char **lasts)
{
	if (strptr)
		*lasts=strptr;

	if ((*lasts)[0]==0)  // 0 length string?
		return(NULL);

	//
	// Note: strcspn & strspn are both called, they're opposites
	//
	int dstart=strcspn(*lasts, delimiters);  // find first char of string in delimiters

	if (dstart == 0)  // string starts with a delimiter
	{
		int dend=strspn(*lasts, delimiters);     // find last char of string NOT in delimiters
		*lasts+=dend;

		if ((*lasts)[0]==0)  // 0 length string?
			return(NULL);

		dstart=strcspn(*lasts, delimiters);
	}
	char *retval=*lasts;

	if ((*lasts)[dstart]==0)  // is this the last token?
		*lasts+=dstart;
	else	// at least one more token to go...
	{
		(*lasts)[dstart]=0;  // null out the end
		*lasts+=(dstart+1);  // advance pointer
	}
	return(retval);
}
#endif