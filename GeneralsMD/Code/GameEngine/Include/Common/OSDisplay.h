// Overridable.h //////////////////////////////////////////////////////////////////////////////////
// Electronic Arts Pacific
// Do Not Distribute

#pragma once

#ifndef __OSDISPLAY_H__
#define __OSDISPLAY_H__

#include "Lib/Basetype.h"

class AsciiString;

enum OSDisplayButtonType
{
	OSDBT_OK										= 0x00000001,
	OSDBT_CANCEL								= 0x00000002,
															
															
	OSDBT_ERROR									= 0x80000000
};

enum OSDisplayOtherFlags
{
	OSDOF_SYSTEMMODAL						= 0x00000001,
	OSDOF_APPLICATIONMODAL			= 0x00000002,
	OSDOF_TASKMODAL							= 0x00000004,
	
	OSDOF_EXCLAMATIONICON				= 0x00000008,
	OSDOF_INFORMATIONICON				= 0x00000010,
	OSDOF_ERRORICON							= 0x00000011,
	OSDOF_STOPICON							= 0x00000012,

	ODDOF_ERROR									= 0x80000000
};

// Display a warning box to the user with the specified localized prompt, message, and
// buttons. (Feel free to add buttons as appropriate to the enum above). 
// This function will return the button pressed to close the dialog.
OSDisplayButtonType OSDisplayWarningBox(AsciiString p, AsciiString m, UnsignedInt buttonFlags, UnsignedInt otherFlags);

#endif /* __OSDISPLAY_H__ */