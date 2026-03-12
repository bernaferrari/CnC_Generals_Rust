// FILE: SwayClientUpdate.h ////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, May 2002
// Desc:   Tree sway client update module
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __SWAYCLIENTUPDATE_H_
#define __SWAYCLIENTUPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/ClientUpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
/** The tree way client update module */
//-------------------------------------------------------------------------------------------------
class SwayClientUpdate : public ClientUpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SwayClientUpdate, "SwayClientUpdate" )
	MAKE_STANDARD_MODULE_MACRO( SwayClientUpdate );

public:

	SwayClientUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	/// the client update callback
	virtual void clientUpdate( void );

	void stopSway( void ) { m_swaying = false; }

protected:

	Real			m_curValue;
	Real			m_curAngle;
	Real			m_curDelta;	 
	Real			m_curAngleLimit;
	Real			m_leanAngle;							///<Angle that the tree leans away from the wind.
	Short			m_curVersion;
	Bool			m_swaying;
	Bool			m_unused;

	void updateSway(void);
};

#endif // __SWAYCLIENTUPDATE_H_

