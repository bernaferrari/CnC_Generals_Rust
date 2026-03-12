// ObjectScriptStatusBits.h ///////////////////////////////////////////////////////////////////////
// Part of header detangling
// JKMCD Aug 2002

#pragma once
#ifndef __OBJECTSCRIPTSTATUSBITS_H__
#define __OBJECTSCRIPTSTATUSBITS_H__

//-------------------------------------------------------------------------------------------------
/** Object status bits */
//-------------------------------------------------------------------------------------------------
enum ObjectScriptStatusBit
{
	OBJECT_STATUS_SCRIPT_DISABLED						= 0x01,		///< this object is disabled via script
	OBJECT_STATUS_SCRIPT_UNPOWERED					= 0x02,		///< this object is unpowered via script
	OBJECT_STATUS_SCRIPT_UNSELLABLE					= 0x04,		///< this object is unsellable
	OBJECT_STATUS_SCRIPT_UNSTEALTHED				= 0x08,		///< this object can't stealth.
	OBJECT_STATUS_SCRIPT_TARGETABLE					= 0x10,  ///< This unit can be targeted by the player, but not autoacquired.
	// NOTE: Object currently only uses a Byte for this, so if you add status bits, you may need to enlarge that field.
};

#endif /* __OBJECTSCRIPTSTATUSBITS_H__ */

