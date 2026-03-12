#if defined(_MSC_VER)
#pragma once
#endif

#ifndef CASTRES_H
#define CASTRES_H

#include "always.h"
#include "vector3.h"
#include "bittype.h"


/**
** CastResultStruct
** Result of a volume or ray cast operation will be stored in the following structure
** NOTE: If you can avoid it, do not enable ComputeContactPoint.  When casting rays, it is more 
** efficient to use the resulting Fraction to compute the contact point outside of the 
** collision detection code.  In the case of AABox sweeping for character collision detection,
** you don't usually need the actual point of contact, etc etc.  
**
** The default state of ComputeContactPoint is *false*
*/
struct CastResultStruct
{
	CastResultStruct(void)	{ Reset(); }
	void		Reset(void)		{ StartBad = false; Fraction = 1.0f; Normal.Set(0,0,0); SurfaceType = 0; ComputeContactPoint = false; ContactPoint.Set(0,0,0); }
	
	bool		StartBad;		// was the inital configuration interpenetrating something?
	float		Fraction;		// fraction of the move up until collision
	Vector3	Normal;			// surface normal at the collision point
	uint32	SurfaceType;	// surface type of polygon at collision point (see W3D_SURFACE_TYPES in w3d_file.h)

	bool		ComputeContactPoint;		// This signals the collision code to compute the point of collision
	Vector3	ContactPoint;				// This will be set to the point of collision if ComputeContactPoint is true

};


#endif
