// FILE: RailedTransportContain.h /////////////////////////////////////////////////////////////////
// Author: Colin Day, August 2002
// Desc: Railed transport contain module
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __RAILED_TRANSPORT_CONTAIN_H_
#define __RAILED_TRANSPORT_CONTAIN_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/TransportContain.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class RailedTransportContain : public TransportContain
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( RailedTransportContain, "RailedTransportContain" )
	MAKE_STANDARD_MODULE_MACRO( RailedTransportContain )

public:

	RailedTransportContain( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onRemoving( Object *obj );			///< object no longer contains 'obj'
	virtual void exitObjectViaDoor( Object *newObj, ExitDoorType exitDoor );
	virtual void exitObjectByBudding( Object *newObj, Object *budHost ) { return; };

protected:

	virtual Bool isSpecificRiderFreeToExit( Object *obj );

};

#endif  // end __RAILED_TRANSPORT_CONTAIN_H_
