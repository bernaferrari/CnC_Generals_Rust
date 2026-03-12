#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __DEFINITION_CLASS_ID_H
#define __DEFINITION_CLASS_ID_H

#include "always.h"
#include "bittype.h"

//////////////////////////////////////////////////////////////////////////////////
//
//	Constants
//
//////////////////////////////////////////////////////////////////////////////////
const int	DEF_CLASSID_START			= 0x00001000;
const int	DEF_CLASSID_RANGE			= 0x00001000;
#define		NEXT_SUPER_CLASSID(n)	DEF_CLASSID_START + (n * DEF_CLASSID_RANGE)


//////////////////////////////////////////////////////////////////////////////////
//
//	DefinitionClassID
//
//	Note:  The following enum should contain ALL of the class ids for 
// definitions in the entire system (to guarantee they are unique).  Each
// super-class is allocated a range of class ids.  Use the 
// SuperClassID_From_ClassID function to determine which super class
//	a particular class id belongs to.
//
//////////////////////////////////////////////////////////////////////////////////
enum
{
	CLASSID_TERRAIN			= NEXT_SUPER_CLASSID(0),
	CLASSID_TILE				= NEXT_SUPER_CLASSID(1),
	CLASSID_GAME_OBJECTS		= NEXT_SUPER_CLASSID(2),
	CLASSID_LIGHT				= NEXT_SUPER_CLASSID(3),
	CLASSID_SOUND				= NEXT_SUPER_CLASSID(4),
	CLASSID_WAYPATH			= NEXT_SUPER_CLASSID(5),
	CLASSID_ZONE				= NEXT_SUPER_CLASSID(6),
	CLASSID_TRANSITION		= NEXT_SUPER_CLASSID(7),
	CLASSID_PHYSICS			= NEXT_SUPER_CLASSID(8),
	CLASSID_EDITOR_OBJECTS	= NEXT_SUPER_CLASSID(9),
	CLASSID_MUNITIONS			= NEXT_SUPER_CLASSID(10),
	CLASSID_DUMMY_OBJECTS	= NEXT_SUPER_CLASSID(11),
	CLASSID_BUILDINGS			= NEXT_SUPER_CLASSID(12),
	CLASSID_TWIDDLERS			= NEXT_SUPER_CLASSID(13),
	CLASSID_GLOBAL_SETTINGS	= NEXT_SUPER_CLASSID(14),
};


//////////////////////////////////////////////////////////////////////////////////
//
//	SuperClassID_From_ClassID
//
//////////////////////////////////////////////////////////////////////////////////
inline uint32
SuperClassID_From_ClassID (uint32 class_id)
{
	//
	//	Which id-range does it fall under?
	//
	int delta			= class_id - DEF_CLASSID_START;
	int num_ranges		= delta / DEF_CLASSID_RANGE;

	return DEF_CLASSID_START + (num_ranges * DEF_CLASSID_RANGE);
}


#endif //__DEFINITION_CLASS_ID_H

