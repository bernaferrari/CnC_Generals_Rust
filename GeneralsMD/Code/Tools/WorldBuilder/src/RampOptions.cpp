// FILE: RampOptions.cpp 
/*---------------------------------------------------------------------------*/
/* EA Pacific                                                                */
/* Confidential Information	                                                 */
/* Copyright (C) 2001 - All Rights Reserved                                  */
/* DO NOT DISTRIBUTE                                                         */
/*---------------------------------------------------------------------------*/
/* Project:    RTS3                                                          */
/* File name:  RampOptions.cpp                                               */
/* Created:    John K. McDonald, Jr., 4/23/2002                              */
/* Desc:       // Ramp options. Contains the Apply button                    */
/* Revision History:                                                         */
/*		4/23/2002 : Initial creation                                           */
/*---------------------------------------------------------------------------*/

#include "StdAfx.h"
#include "RampOptions.h"

RampOptions::RampOptions(CWnd* pParent) : COptionsPanel(RampOptions::IDD, pParent)
{
	if (TheRampOptions) {
		// oh shit.
		return;
	}

	TheRampOptions = this;
	m_rampWidth = 20;
	m_shouldApplyTheRamp = false;
}

RampOptions::~RampOptions()
{
	TheRampOptions = NULL;
}

Bool RampOptions::shouldApplyTheRamp()
{
	if (m_shouldApplyTheRamp) {
		m_shouldApplyTheRamp = false;
		return true;
	}

	return false;
}

void RampOptions::OnApply()
{
	// Set m_shouldApplyTheRamp true. The call to shouldApplyRamp will set it false
	m_shouldApplyTheRamp = true;
}

void RampOptions::OnWidthChange()
{
	CString str;
	CWnd* pWnd = GetDlgItem(IDC_RO_WIDTH);
	if (!pWnd) {
		return;
	}

	pWnd->GetWindowText(str);
	m_rampWidth = atof(str.GetBuffer(0));
}

extern RampOptions* TheRampOptions = NULL;

BEGIN_MESSAGE_MAP(RampOptions, COptionsPanel)
	ON_BN_CLICKED(IDC_RO_APPLY, OnApply)
	ON_EN_CHANGE(IDC_RO_WIDTH, OnWidthChange)
END_MESSAGE_MAP()
