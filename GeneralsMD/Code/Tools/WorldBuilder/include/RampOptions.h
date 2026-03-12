// FILE: RampOptions.h 
/*---------------------------------------------------------------------------*/
/* EA Pacific                                                                */
/* Confidential Information	                                                 */
/* Copyright (C) 2001 - All Rights Reserved                                  */
/* DO NOT DISTRIBUTE                                                         */
/*---------------------------------------------------------------------------*/
/* Project:    RTS3                                                          */
/* File name:  RampOptions.h                                                 */
/* Created:    John K. McDonald, Jr., 4/23/2002                              */
/* Desc:       // Apply button for ramps.                                    */
/* Revision History:                                                         */
/*		4/23/2002 : Initial creation                                           */
/*---------------------------------------------------------------------------*/

#pragma once
#ifndef _H_RAMPOPTIONS_
#define _H_RAMPOPTIONS_

// INCLUDES ///////////////////////////////////////////////////////////////////
#include "OptionsPanel.h"
#include "Resource.h"

// DEFINES ////////////////////////////////////////////////////////////////////
// TYPE DEFINES ///////////////////////////////////////////////////////////////
// FORWARD DECLARATIONS ///////////////////////////////////////////////////////

class RampOptions : public COptionsPanel
{
	Bool m_shouldApplyTheRamp;
	Real m_rampWidth;
	public:
		enum { IDD = IDD_RAMP_OPTIONS };
		RampOptions(CWnd* pParent = NULL);
		virtual ~RampOptions();

		Bool shouldApplyTheRamp();
		Real getRampWidth() { return m_rampWidth; }

		afx_msg void OnApply();
		afx_msg void OnWidthChange();

	DECLARE_MESSAGE_MAP()
};

extern RampOptions* TheRampOptions;

#endif /* _H_RAMPOPTIONS_ */
