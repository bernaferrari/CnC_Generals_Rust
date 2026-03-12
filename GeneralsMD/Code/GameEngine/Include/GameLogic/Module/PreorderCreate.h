// FILE: PreorderCreate.h /////////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, December 2002
// Desc:   When a building is created, set the preorder status if necessary
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _PREORDER_CREATE_H_
#define _PREORDER_CREATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/CreateModule.h"

class Thing;

//-------------------------------------------------------------------------------------------------
/** PreorderCreate */
//-------------------------------------------------------------------------------------------------
class PreorderCreate : public CreateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( PreorderCreate, "PreorderCreate" )
	MAKE_STANDARD_MODULE_MACRO( PreorderCreate )

public:

	PreorderCreate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onCreate( void );
	virtual void onBuildComplete( void );

protected:

};

#endif // _PREORDER_CREATE_H_
