#ifndef COLMATHPLANE_H
#define COLMATHPLANE_H

#include "always.h"
#include "plane.h"
#include "aabox.h"

/*
** Inline collision functions dealing with planes
** This module is meant to be included only in .CPP files after you include colmath.h
** It is not automatically included in order to reduce file dependencies...
*/

/***********************************************************************************************
 * get_far_extent -- gets extents of a box projected onto an axis                              *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   3/29/2000  gth : Created.                                                                 *
 *=============================================================================================*/
inline void get_far_extent(const Vector3 & normal,const Vector3 & extent,Vector3 * posfarpt)
{
	if (WWMath::Fast_Is_Float_Positive(normal.X)) {
		posfarpt->X = extent.X;
	} else {
		posfarpt->X = -extent.X;
	}

	if (WWMath::Fast_Is_Float_Positive(normal.Y)) {
		posfarpt->Y = extent.Y;
	} else {
		posfarpt->Y = -extent.Y;
	}

	if (WWMath::Fast_Is_Float_Positive(normal.Z)) {
		posfarpt->Z = extent.Z;
	} else {
		posfarpt->Z = -extent.Z;
	}
}


/***********************************************************************************************
 * CollisionMath::Overlap_Test -- Tests overlap between a plane and a point                    *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   3/29/2000  gth : Created.                                                                 *
 *=============================================================================================*/
inline 
CollisionMath::OverlapType
CollisionMath::Overlap_Test(const PlaneClass & plane,const Vector3 & point)
{
	float delta = Vector3::Dot_Product(point,plane.N) - plane.D;
	if (delta > COINCIDENCE_EPSILON) {
		return POS;
	} 
	if (delta < -COINCIDENCE_EPSILON) {
		return NEG;
	}
	return ON;
}


/***********************************************************************************************
 * CollisionMath::Overlap_Test -- Tests overlap between a plane and an AABox                   *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   3/29/2000  gth : Created.                                                                 *
 *=============================================================================================*/
inline
CollisionMath::OverlapType
CollisionMath::Overlap_Test(const PlaneClass & plane,const AABoxClass & box)
{
	// First, we determine the the near and far points of the box in the
	// direction of the plane normal
	Vector3 posfarpt;
	Vector3 negfarpt;

	get_far_extent(plane.N,box.Extent,&posfarpt);
	
	negfarpt = -posfarpt;
	posfarpt += box.Center;
	negfarpt += box.Center;
	if (Overlap_Test(plane,negfarpt) == POS) {
		return POS;
	}
	if (Overlap_Test(plane,posfarpt) == NEG) {
		return NEG;
	}
	return BOTH;
}


#endif

