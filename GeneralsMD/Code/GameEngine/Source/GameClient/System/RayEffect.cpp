// FILE: RayEffect.cpp ////////////////////////////////////////////////////////////////////////////
// Created:   Colin Day, May 2001
// Desc:      Ray effect system manager
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameClient/RayEffect.h"
#include "GameClient/Drawable.h"

// PUBLIC DATA ////////////////////////////////////////////////////////////////////////////////////
class RayEffectSystem *TheRayEffects = NULL;

// PRIVATE METHODS ////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
/** Find an effect entry given a drawable */
//-------------------------------------------------------------------------------------------------
RayEffectData *RayEffectSystem::findEntry( const Drawable *draw )
{
	Int i;
	RayEffectData *effectData = NULL;

	// find the matching effect data entry
	for( i = 0; i < MAX_RAY_EFFECTS; i++ )
	{

		if( m_effectData[ i ].draw == draw )
		{

			effectData = &m_effectData[ i ];
			break;  // exit for i

		}  // end if

	}  // end for i

	return effectData;

}  // end findEntry

// PUBLIC METHODS /////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
RayEffectSystem::RayEffectSystem( void )
{

	init();

}  // end RayEffectSystem

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
RayEffectSystem::~RayEffectSystem( void )
{

}  // end ~RayEffectSystem

//-------------------------------------------------------------------------------------------------
/** initialize the system */
//-------------------------------------------------------------------------------------------------
void RayEffectSystem::init( void )
{
	Int i;

	for( i = 0; i < MAX_RAY_EFFECTS; i++ )
	{

		m_effectData[ i ].draw = NULL;
		m_effectData[ i ].startLoc.zero();
		m_effectData[ i ].endLoc.zero();

	}  // end for i

}  // end init

//-------------------------------------------------------------------------------------------------
/** Reset */
//-------------------------------------------------------------------------------------------------
void RayEffectSystem::reset( void )
{

	// nothing dynamic going on here, just initialize it
	init();

}  // end reset

//-------------------------------------------------------------------------------------------------
/** add a ray effect entry for this drawable */
//-------------------------------------------------------------------------------------------------
void RayEffectSystem::addRayEffect( const Drawable *draw, 
																	  const Coord3D *startLoc, 
																	  const Coord3D *endLoc )
{
	Int i;
	RayEffectData *effectData = NULL;

	// sanity
	if( draw == NULL || startLoc == NULL || endLoc == NULL )	
		return;

	/** @todo this should be more intelligent and should not be limited
	to any kind of max ray effects, this is all a temporary hack system for
	the demo anyway right now though */

	// search for a free effect slot
	for( i = 0; i < MAX_RAY_EFFECTS; i++ )
	{

		if( m_effectData[ i ].draw == NULL )	
		{

			effectData = &m_effectData[ i ];
			break;  // exit for 

		}  // end if

	}  // end for i

	// if no free slots we can't do it
	if( effectData == NULL )
		return;

	// add the data to the entry
	effectData->draw = draw;
	effectData->startLoc = *startLoc;
	effectData->endLoc = *endLoc;

}

//-------------------------------------------------------------------------------------------------
/** given a drawable, remove its effect from the system */
//-------------------------------------------------------------------------------------------------
void RayEffectSystem::deleteRayEffect( const Drawable *draw )
{
	RayEffectData *effectData = NULL;

	// sanity
	if( draw == NULL )
		return;

	// find the effect entry
	effectData = findEntry( draw );
	if( effectData )
	{

		// remove the data for this entry
		effectData->draw = NULL;

	}  // end if

}  // end deleteRayEffect

//-------------------------------------------------------------------------------------------------
/** given a drawable, if it is in the ray effect system list retrieve
	*	the ray effect data for its entry */
//-------------------------------------------------------------------------------------------------
void RayEffectSystem::getRayEffectData( const Drawable *draw, 
																			  RayEffectData *effectData )
{
	RayEffectData *entry = NULL;

	// sanity
	if( draw == NULL || effectData == NULL )
		return;

	// find the effect data entry
	entry = findEntry( draw );
	if( entry )
	{

		// data has been found, copy to parameter
		*effectData = *entry;

	}  // end effectData

}  // end getRayEffectData

