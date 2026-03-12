#include "always.h"
#include "string.h"
#include "nstrdup.h"
//#include "../app/main/gnew.h"

/************************************************************************** 
 * nstrdup -- duplicates a string using new[]                             * 
 *                                                                        * 
 * INPUT:                                                                 * 
 *                                                                        * 
 * OUTPUT:                                                                * 
 *                                                                        * 
 * WARNINGS:                                                              * 
 *                                                                        * 
 * HISTORY:                                                               * 
 *   02/03/1998 PWG : Created.                                            * 
 *========================================================================*/
char * nstrdup(const char *str)
{
	if(str == 0) return 0;

	// eventually should be replaced with NEW when we go to the wwnew stuff.
	char *retval = W3DNEWARRAY char [strlen(str) + 1];
	strcpy(retval, str);
	return retval;
}


