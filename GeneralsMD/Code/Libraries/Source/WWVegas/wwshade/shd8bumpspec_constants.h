// DX8 bump specular mask with gloss map shader constants
// Kenny Mitchell - Westwood Studios EA 2002

#ifndef SHD8BUMPSPEC_CONSTANTS_H
#define SHD8BUMPSPEC_CONSTANTS_H

// vertex shader macros


#define CV_WORLD_VIEW_PROJECTION			1

#define CV_WORLD_VIEW_PROJECTION_0		1
#define CV_WORLD_VIEW_PROJECTION_1		2
#define CV_WORLD_VIEW_PROJECTION_2		3
#define CV_WORLD_VIEW_PROJECTION_3		4


#define CV_WORLD								12

#define CV_WORLD_0							12
#define CV_WORLD_1							13
#define CV_WORLD_2							14
#define CV_WORLD_3							15

// 16-26 lighting constants

#define CV_BUMPINESS							27

#define CV_EYE_WORLD							28

#define CV_TEXMAP								30

#define CV_TEXMAP_0							30
#define CV_TEXMAP_1							31
#define CV_TEXMAP_2							32
#define CV_TEXMAP_3							33

// inputs
#define V_POSITION							v0
#define V_NORMAL								v1
#define V_DIFFUSE								v2
#define V_TEXTURE								v3
#define V_S										v4
#define V_T										v5
#define V_SxT									v6


// registers
#define HALF_ANGLE			r0

#define S_WORLD				r1

#define T_WORLD				r2
#define SxT_WORLD				r3
#define LIGHT_LOCAL			r4
#define LIGHT_0				r5
#define LIGHT_1				r6
#define LIGHT_2				r7
#define LIGHT_3				r8
#define COL						r9
#define WORLD_NORMAL			r10

#define EYE_VECTOR			r11
#define WORLD_VERTEX			r11


// pixel shader constants

#define OUTPUT_REG			r0

// texture stages
#define TEX_NORMALMAP	t0
#define TEX_DECAL			t1
#define TEX_SPECULAR		t2

#define COL_LIGHT			v0
#define COL_DIFFUSE		v1



#endif
