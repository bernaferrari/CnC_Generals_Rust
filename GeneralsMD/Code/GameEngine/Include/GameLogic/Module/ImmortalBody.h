// FILE: ImmortalBody.h ////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, April 2002
// Desc:	 Just like Active Body, but won't let health drop below 1
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __IMMORTAL_BODY_H
#define __IMMORTAL_BODY_H

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/ActiveBody.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Object;

//-------------------------------------------------------------------------------------------------
/** Structure body module */
//-------------------------------------------------------------------------------------------------
class ImmortalBody : public ActiveBody
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ImmortalBody, "ImmortalBody" )
	MAKE_STANDARD_MODULE_MACRO( ImmortalBody )

public:

	ImmortalBody( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void internalChangeHealth( Real delta );								///< change health

protected:

};

#endif // __STRUCTUREBODY_H_

