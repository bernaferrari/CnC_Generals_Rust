// FILE: SquishCollide.h ////////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, Jan 2002
// Desc:   Topple collide module
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __SquishCollide_H_
#define __SquishCollide_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/CollideModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
/** The tank collide module */
//-------------------------------------------------------------------------------------------------
class SquishCollide : public CollideModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SquishCollide, "SquishCollide" )
	MAKE_STANDARD_MODULE_MACRO( SquishCollide );

public:

	SquishCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	/// This collide method gets called when collision occur
	virtual void onCollide( Object *other, const Coord3D *loc, const Coord3D *normal );

protected:

};

#endif // __SquishCollide_H_

