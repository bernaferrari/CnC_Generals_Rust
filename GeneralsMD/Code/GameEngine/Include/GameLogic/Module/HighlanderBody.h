// FILE: HighlanderBody.h ////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, November 2002
// Desc:	 Takes damage according to armor, but can't die from normal damage.  Can die from Unresistable though
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __HIGHLANDER_BODY_H
#define __HIGHLANDER_BODY_H

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/ActiveBody.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Object;

//-------------------------------------------------------------------------------------------------
/** Structure body module */
//-------------------------------------------------------------------------------------------------
class HighlanderBody : public ActiveBody
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( HighlanderBody, "HighlanderBody" )
	MAKE_STANDARD_MODULE_MACRO( HighlanderBody )

public:

	HighlanderBody( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void attemptDamage( DamageInfo *damageInfo );		///< try to damage this object

protected:

};

#endif 

