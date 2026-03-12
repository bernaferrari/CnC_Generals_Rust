// FILE: Powers.h /////////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, November 2001
// Desc:	 Unit power definitions
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __POWERS_H_
#define __POWERS_H_

//
// skeleton definition of unit powers
//

// power bit flags, keep this in sync with PowerNames
enum
{
	POWER_NONE = 0,				// 0x0000000000000000 
	POWER_FASTER,					// 0x0000000000000001 
	POWER_DOUBLE_SHOT,		// 0x0000000000000002 
	POWER_SELF_HEALING,		// 0x0000000000000004 

	POWERS_NUM_POWERS
};

#ifdef DEFINE_POWER_NAMES
static char *PowerNames[] = 
{
	"NONE",
	"FASTER",
	"DOUBLE_SHOT",
	"SELF_HEALING",
	NULL
};
#endif  // end DEFINE_POWER_NAMES

#endif // __POWERS_H_

