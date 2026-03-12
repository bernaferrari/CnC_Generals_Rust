#ifndef BONEICON_H
#define BONEICON_H

struct VertexStruct
{
	float X,Y,Z;
};

struct FaceStruct
{
	int V0,V1,V2;
};

extern const int NumBoneIconVerts;
extern const int NumBoneIconFaces;
extern VertexStruct BoneIconVerts[];
extern FaceStruct BoneIconFaces[];

#endif
