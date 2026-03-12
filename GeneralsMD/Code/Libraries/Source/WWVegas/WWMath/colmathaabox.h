#if defined(_MSC_VER)
#pragma once
#endif

#ifndef COLMATHAABOX_H
#define COLMATHAABOX_H

#include "always.h"
#include "aabox.h"
#include "vector3.h"
#include "lineseg.h"


/***********************************************************************************************
 * CollisionMath::Overlap_Test -- test overlap between an AABox and a point                    *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   3/14/2000  gth : Created.                                                                 *
 *=============================================================================================*/
WWINLINE CollisionMath::OverlapType CollisionMath::Overlap_Test(const AABoxClass & box,const Vector3 & point)
{
	if (WWMath::Fabs(point.X - box.Center.X) > box.Extent.X) return POS;
	if (WWMath::Fabs(point.Y - box.Center.Y) > box.Extent.Y) return POS;
	if (WWMath::Fabs(point.Z - box.Center.Z) > box.Extent.Z) return POS;

	return NEG;
}

/***********************************************************************************************
 * CollisionMath::Overlap_Test -- Tests overlap between two AABoxes                            *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   11/19/99   gth : Created.                                                                 *
 *=============================================================================================*/
WWINLINE CollisionMath::OverlapType CollisionMath::Overlap_Test(const AABoxClass & box,const AABoxClass & box2)
{
	Vector3 dc;
	Vector3::Subtract(box2.Center,box.Center,&dc);

	if (box.Extent.X + box2.Extent.X < WWMath::Fabs(dc.X)) return POS;
	if (box.Extent.Y + box2.Extent.Y < WWMath::Fabs(dc.Y)) return POS;
	if (box.Extent.Z + box2.Extent.Z < WWMath::Fabs(dc.Z)) return POS;

	if (	(dc.X + box2.Extent.X <= box.Extent.X) &&
			(dc.Y + box2.Extent.Y <= box.Extent.Y) &&
			(dc.Z + box2.Extent.Z <= box.Extent.Z) &&
			(dc.X - box2.Extent.X >= -box.Extent.X) &&
			(dc.Y - box2.Extent.Y >= -box.Extent.Y) &&
			(dc.Z - box2.Extent.Z >= -box.Extent.Z))
	{
		return NEG;	// inside;
	}

	return BOTH;
}

#endif

