// FILE: RayEffect.h //////////////////////////////////////////////////////////////////////////////
// Created:    Colin Day, May 2001
// Desc:			 Ray effect manager
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __RAYEFFECT_H_
#define __RAYEFFECT_H_

// INCLUDE ////////////////////////////////////////////////////////////////////////////////////////
#include "Lib/BaseType.h"
#include "Common/SubsystemInterface.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Drawable;

//-------------------------------------------------------------------------------------------------
/** Data the ray effect system keeps for an entry */
//-------------------------------------------------------------------------------------------------
struct RayEffectData
{

	const Drawable *draw;  ///< the drawable
	Coord3D startLoc;  ///< start location for ray
	Coord3D endLoc;  ///< end location for ray

};  // end RayEffectData

//-------------------------------------------------------------------------------------------------
/** This class maintains all the ray effects visible in the world */
//-------------------------------------------------------------------------------------------------
class RayEffectSystem : public SubsystemInterface
{

public:

	RayEffectSystem( void );
	~RayEffectSystem( void );

	virtual void init( void );
	virtual void reset( void );
	virtual void update( void ) { }

	/// add a ray effect entry for this drawable
	void addRayEffect( const Drawable *draw, const Coord3D *startLoc, const Coord3D *endLoc );

	/// given a drawable, remove its effect from the system
	void deleteRayEffect( const Drawable *draw );

	/** given a drawable, if it is in the ray effect system list retrieve
	the ray effect data for its entry */
	void getRayEffectData( const Drawable *draw, RayEffectData *effectData );

protected:

	/// find an effect data entry based on the drawable
	RayEffectData *findEntry( const Drawable *draw );

	enum
	{
		MAX_RAY_EFFECTS = 128
	};
	RayEffectData m_effectData[ MAX_RAY_EFFECTS ];  ///< all the ray effects

};  // end RayEffectSystem

// EXTERN /////////////////////////////////////////////////////////////////////////////////////////
extern RayEffectSystem *TheRayEffects;  ///< the ray effects singleton external

#endif // $label

