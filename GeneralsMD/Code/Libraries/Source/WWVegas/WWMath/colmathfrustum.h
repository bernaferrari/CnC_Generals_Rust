#if defined(_MSC_VER)
#pragma once
#endif

#ifndef COLMATHFRUSTUM_H
#define COLMATHFRUSTUM_H

#include "always.h"
#include "aabox.h"
#include "vector3.h"
#include "lineseg.h"
#include "frustum.h"

/*
** Inline collision functions dealing with frustums
*/

/***********************************************************************************************
 * CollisionMath::Overlap_Test -- test a frustum and an AABox for overlap                      *
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
CollisionMath::Overlap_Test(const FrustumClass & frustum,const AABoxClass & box,int & planes_passed)
{
	int mask = 0;

	// TODO: doesn't catch all cases...
	for (int i = 0; i < 6; i++) {

		int plane_bit = (1<<i);
	
		// only check this plane if we have to	
		if ((planes_passed & plane_bit) == 0) {
		
			int result = CollisionMath::Overlap_Test(frustum.Planes[i],box);
			if (result == OUTSIDE) {

				return OUTSIDE;
			} else if (result == INSIDE) {
				planes_passed |= plane_bit;
			}
			mask |= result;
		
		} else {
		
			mask |= INSIDE;
		
		}
	}

	if (mask == INSIDE) {
		return INSIDE;
	}
	return OVERLAPPED;
}

#endif
