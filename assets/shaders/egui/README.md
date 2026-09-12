# egui GXP shaders

Unmodified `imgui_v_cg.gxp` and `imgui_f_cg.gxp` from
https://github.com/cy33hc/imgui-vita2d/tree/a849382de1b9e1662ec1440cf7a936f3b4cacff3
(MIT; see LICENSE). Loaded directly by vitaGL, without runtime shader compilation.

Vertex attributes: aPosition (float2), aTexcoord (float2), aColor (float4).
Projection: wvp (float4x4). Fragment shader multiplies sampled texture unit 0 by vertex color.
Frame alpha is applied to premultiplied vertex colors on the CPU.
