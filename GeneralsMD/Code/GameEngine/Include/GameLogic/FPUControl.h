// FILE: FPUControl.h /////////////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, June 2002
// Desc:	 Routines for controlling the FPU state
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __FPUCONTROL_H__
#define __FPUCONTROL_H__

/**
  * setFPMode sets the FPU internal precision and rounding mode.  As DirectX is not guaranteed to
	* leave the FPU in a good state, we must call this at the start of GameLogic::update() and
	* anywhere that touches DirectX inside GameLogic loops (LoadScreen).
	*/
void setFPMode( void );

#endif // __FPUCONTROL_H__
