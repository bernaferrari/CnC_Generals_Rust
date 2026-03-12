#if defined(_MSC_VER)
#pragma once
#endif

#ifndef CULLTYPE_H
#define CULLTYPE_H

#include "always.h"

/*
** CullType is an enumeration of the possible results of a culling 
** operation.  It is placed here so that all of the different cull functions
** (which are scattered throughout WWMath, WW3D, WWPhys, etc) can
** communicate the result in a consistent way
*/

typedef enum CULLTYPE
{
	CULL_OUTSIDE = 0,			// the object was completely outside the culling volume
	CULL_INTERSECTING,		// the object intersects an edge of the culling volume
	CULL_INSIDE					// the object is completely inside the culling volume
};


#endif